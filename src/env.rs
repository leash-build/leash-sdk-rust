//! Runtime env-var primitive — fetches values from the Leash platform.
//!
//! Mirrors `leash.env.get` / `leash.env.getMany` in the TS / Python / Go SDKs.
//! Values are cached for 60 seconds per-instance; the cache is shared between
//! [`Env::get`], [`Env::get_fresh`], and [`Env::get_many`].
//!
//! The Rust surface adopts two adaptations:
//!
//!   * `Result<Option<String>>` — `Ok(None)` for a 404 (key not declared
//!     anywhere), `Err(_)` for everything else. Callers branch with
//!     `if value.is_none()` instead of catching errors.
//!   * Dedicated [`get_fresh`](Self::get_fresh) method for cache-bypass,
//!     rather than an option-struct overload — gives type safety and is
//!     equivalent to the TS `{ fresh: true }` option.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::errors::{LeashError, Result};

/// Default cache window — matches TS/Python/Go.
pub const ENV_CACHE_TTL: Duration = Duration::from_secs(60);

/// `leash.env()` — runtime env-var fetcher with an in-memory TTL cache.
#[derive(Debug, Clone)]
pub struct Env {
    inner: Arc<EnvInner>,
}

#[derive(Debug)]
struct EnvInner {
    platform_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
    cache: Mutex<HashMap<String, Cached>>,
}

#[derive(Debug, Clone)]
struct Cached {
    value: Option<String>,
    expires_at: Instant,
}

impl Env {
    pub(crate) fn new(platform_url: String, api_key: Option<String>, http: reqwest::Client) -> Self {
        Self {
            inner: Arc::new(EnvInner {
                platform_url: platform_url.trim_end_matches('/').to_string(),
                api_key,
                http,
                cache: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Resolve a single env-var by name, using the TTL cache.
    ///
    /// Returns `Ok(None)` when the platform reports the key as not declared
    /// or not found anywhere (HTTP 404). All other failures surface as
    /// [`LeashError`].
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        self.resolve(key, false).await
    }

    /// Like [`Self::get`] but bypasses the cache read. The freshly-fetched
    /// value is still written back to the cache.
    ///
    /// Equivalent to the TS `leash.env.get(key, { fresh: true })` option.
    pub async fn get_fresh(&self, key: &str) -> Result<Option<String>> {
        self.resolve(key, true).await
    }

    /// Resolve multiple env-vars sequentially, sharing the TTL cache.
    ///
    /// If any key fails (auth, plan, network), the whole call returns that
    /// error — partial results are not surfaced (matches Go behaviour). Each
    /// value is `Option<String>` so callers can distinguish "missing" (`None`)
    /// from "empty string" (`Some("")`).
    pub async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Option<String>>> {
        let mut out = HashMap::with_capacity(keys.len());
        for key in keys {
            let value = self.get(key).await?;
            out.insert((*key).to_string(), value);
        }
        Ok(out)
    }

    async fn resolve(&self, key: &str, fresh: bool) -> Result<Option<String>> {
        let now = Instant::now();
        if !fresh {
            if let Some(cached) = self.cache_get(key, now) {
                return Ok(cached);
            }
        }

        let value = self.fetch(key).await?;
        self.cache_put(key, value.clone(), now + ENV_CACHE_TTL);
        Ok(value)
    }

    fn cache_get(&self, key: &str, now: Instant) -> Option<Option<String>> {
        let cache = self.inner.cache.lock().ok()?;
        let entry = cache.get(key)?;
        if entry.expires_at > now {
            Some(entry.value.clone())
        } else {
            None
        }
    }

    fn cache_put(&self, key: &str, value: Option<String>, expires_at: Instant) {
        if let Ok(mut cache) = self.inner.cache.lock() {
            cache.insert(
                key.to_string(),
                Cached {
                    value,
                    expires_at,
                },
            );
        }
    }

    async fn fetch(&self, key: &str) -> Result<Option<String>> {
        let api_key = self.inner.api_key.as_deref().ok_or_else(|| {
            LeashError::Unauthorized {
                message: "LEASH_API_KEY is required to call Env::get.".to_string(),
            }
        })?;

        let url = format!(
            "{}/api/apps/me/secrets/{}",
            self.inner.platform_url,
            percent_encode(key)
        );

        let resp = self
            .inner
            .http
            .get(&url)
            .bearer_auth(api_key)
            .send()
            .await?;

        let status = resp.status();
        let raw = resp.bytes().await?;

        match status.as_u16() {
            400 => Err(LeashError::UpstreamError {
                status: 400,
                message: format!(
                    "Invalid env-var key: '{key}'. Names must match /^[A-Za-z_][A-Za-z0-9_]*$/ and be \u{2264}100 chars."
                ),
            }),
            401 => Err(LeashError::Unauthorized {
                message: "Missing or invalid LEASH_API_KEY.".to_string(),
            }),
            402 => {
                let parsed: Option<serde_json::Value> = serde_json::from_slice(&raw).ok();
                let required_plan = parsed
                    .as_ref()
                    .and_then(|v| v.get("requiredPlan"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let suffix = required_plan
                    .as_deref()
                    .map(|p| format!(" (requiredPlan: {p})"))
                    .unwrap_or_default();
                Err(LeashError::UpgradeRequired {
                    message: format!("Env::get requires the Growth plan or above{suffix}."),
                })
            }
            // Adapted behaviour: 404 → Ok(None) so callers can branch with
            // `if value.is_none()` instead of matching on a specific error.
            404 => Ok(None),
            502 => {
                let parsed: Option<serde_json::Value> = serde_json::from_slice(&raw).ok();
                let msg = parsed
                    .as_ref()
                    .and_then(|v| v.get("error"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Secret source resync failed on the platform side.".into());
                Err(LeashError::UpstreamError {
                    status: 502,
                    message: msg,
                })
            }
            s if s >= 400 => Err(LeashError::UpstreamError {
                status: s,
                message: format!("Unexpected response from platform: HTTP {s}"),
            }),
            _ => {
                let body: SecretBody =
                    serde_json::from_slice(&raw).map_err(|_| LeashError::MalformedResponse {
                        message: format!("Platform returned unexpected shape for key '{key}'."),
                    })?;
                Ok(Some(body.value))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct SecretBody {
    value: String,
}

/// Minimal percent-encoder for path segments — keeps unreserved chars,
/// escapes everything else. Avoids a dep on `percent-encoding` for one call.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encodes_reserved_chars() {
        assert_eq!(percent_encode("OPENAI_API_KEY"), "OPENAI_API_KEY");
        assert_eq!(percent_encode("a/b"), "a%2Fb");
        assert_eq!(percent_encode("a b"), "a%20b");
    }
}
