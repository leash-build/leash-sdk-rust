//! The unified [`Leash`] client — the canonical entry point.
//!
//! Mirrors the TS `Leash`, Python `Leash`, and Go `Client`. One async surface
//! for identity, runtime env vars, and integrations.

use crate::auth::{Auth, LEASH_AUTH_COOKIE};
use crate::env::Env;
use crate::errors::{LeashError, Result};
use crate::integrations::Integrations;
use crate::request::LeashRequest;
use crate::transport::Transport;

/// Default Leash platform URL.
pub const DEFAULT_PLATFORM_URL: &str = "https://leash.build";

/// `Authorization: Bearer …` header — captured for the env-fetch fallback,
/// **never** forwarded on integration POSTs (see [`Transport`]).
const AUTHORIZATION_HEADER: &str = "Authorization";

/// Unified Leash client.
///
/// Construct from any HTTP request that implements [`LeashRequest`]:
///
/// ```no_run
/// # async fn ex() -> leash_sdk::Result<()> {
/// use leash_sdk::Leash;
/// let req = http::Request::builder().body(()).unwrap();
/// let leash = Leash::new(&req)?;
/// let user = leash.auth().user().await?;
/// # let _ = user; Ok(()) }
/// ```
///
/// Or for server-to-server flows:
///
/// ```no_run
/// # fn ex() -> leash_sdk::Result<()> {
/// use leash_sdk::Leash;
/// let leash = Leash::from_api_key("lsk_live_…")?;
/// # let _ = leash; Ok(()) }
/// ```
///
/// Or from a raw JWT held by a CLI / agent:
///
/// ```no_run
/// # fn ex() -> leash_sdk::Result<()> {
/// use leash_sdk::Leash;
/// let leash = Leash::from_token("eyJhbGciOi…")?;
/// # let _ = leash; Ok(()) }
/// ```
///
/// ## Auth precedence
///
/// Matches the TS / Python / Go surface:
///
/// 1. `LEASH_API_KEY` env var (or [`Leash::with_api_key`]) — the server key
///    used by integration POSTs (`X-API-Key`) and env-fetch (`Authorization: Bearer`).
/// 2. `Authorization: Bearer <jwt>` on the inbound request — used for
///    `/api/auth/me` and, **only when no API key is configured**, as a
///    fallback bearer on env-fetch endpoints. **Never** forwarded on
///    integration POSTs — the platform's `verifyToken()` would reject a user
///    JWT before the API-key check runs.
/// 3. `leash-auth` cookie — forwarded to the platform for integration calls.
#[derive(Debug, Clone)]
pub struct Leash {
    platform_url: String,
    api_key: Option<String>,
    bearer_token: Option<String>,
    cookie_value: Option<String>,
    http: reqwest::Client,
}

impl Leash {
    /// Construct from an inbound HTTP request.
    ///
    /// The request's `leash-auth` cookie and `Authorization: Bearer` header
    /// are extracted; `LEASH_API_KEY` is read from the environment.
    ///
    /// **Bearer → env-fetch fallback**: when no `LEASH_API_KEY` is configured,
    /// an inbound `Authorization: Bearer` token is used to authorise
    /// `env.get` calls. This lets a JWT-only caller (e.g. a script holding a
    /// user token but no server key) still resolve env vars. Bearer is
    /// **never** forwarded on integration POSTs.
    pub fn new<R: LeashRequest>(req: R) -> Result<Self> {
        let cookie_value = req.cookie(LEASH_AUTH_COOKIE);
        let bearer_token = req
            .header(AUTHORIZATION_HEADER)
            .and_then(extract_bearer);

        Ok(Self {
            platform_url: resolve_platform_url(None),
            api_key: std::env::var("LEASH_API_KEY").ok().filter(|s| !s.is_empty()),
            bearer_token,
            cookie_value,
            http: default_http_client(),
        })
    }

    /// Construct a server-to-server client from an explicit API key.
    ///
    /// The key is sent as `X-API-Key` on integration calls and as
    /// `Authorization: Bearer` on env-fetch calls. No `leash-auth` cookie is
    /// set — integration calls that require user context will return a
    /// [`LeashError::Unauthorized`].
    pub fn from_api_key(api_key: impl Into<String>) -> Result<Self> {
        let key = api_key.into();
        if key.is_empty() {
            return Err(LeashError::Unauthorized {
                message: "LEASH_API_KEY is empty.".to_string(),
            });
        }
        Ok(Self {
            platform_url: resolve_platform_url(None),
            api_key: Some(key),
            bearer_token: None,
            cookie_value: None,
            http: default_http_client(),
        })
    }

    /// Construct from a raw user JWT (CLI / agent flows).
    ///
    /// The token is forwarded as the `leash-auth` cookie value on integration
    /// calls and is used for identity reads via [`Self::auth`].
    pub fn from_token(token: impl Into<String>) -> Result<Self> {
        let tok = token.into();
        if tok.is_empty() {
            return Err(LeashError::Unauthorized {
                message: "JWT is empty.".to_string(),
            });
        }
        Ok(Self {
            platform_url: resolve_platform_url(None),
            api_key: std::env::var("LEASH_API_KEY").ok().filter(|s| !s.is_empty()),
            bearer_token: Some(tok.clone()),
            cookie_value: Some(tok),
            http: default_http_client(),
        })
    }

    /// Override the platform base URL (defaults to `LEASH_PLATFORM_URL` env
    /// var or `https://leash.build`).
    #[must_use]
    pub fn with_platform_url(mut self, url: impl Into<String>) -> Self {
        self.platform_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Provide an explicit API key, overriding the env var.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Inject a custom [`reqwest::Client`] (useful for tests + custom transports).
    #[must_use]
    pub fn with_http_client(mut self, http: reqwest::Client) -> Self {
        self.http = http;
        self
    }

    // ----------------------------------------------------------------------
    // Namespaces
    // ----------------------------------------------------------------------

    /// `leash.auth()` — identity reads (non-throwing).
    pub fn auth(&self) -> Auth {
        Auth {
            cookie: self.cookie_value.clone(),
        }
    }

    /// `leash.env()` — runtime env-var fetcher with TTL cache.
    pub fn env(&self) -> Env {
        // Bearer → env-fetch fallback (documented in [`Self::new`]).
        let key = self
            .api_key
            .clone()
            .or_else(|| self.bearer_token.clone());
        Env::new(self.platform_url.clone(), key, self.http.clone())
    }

    /// `leash.integrations()` — typed providers + generic escape hatch.
    pub fn integrations(&self) -> Integrations {
        Integrations::new(self.transport())
    }

    // ----------------------------------------------------------------------
    // Inspectors (handy for tests + debug)
    // ----------------------------------------------------------------------

    /// The resolved platform base URL.
    pub fn platform_url(&self) -> &str {
        &self.platform_url
    }

    /// Returns `true` when an `X-API-Key` will be sent on integration calls.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Returns `true` when a `leash-auth` cookie will be forwarded on integration calls.
    pub fn has_cookie(&self) -> bool {
        self.cookie_value.is_some()
    }

    /// Returns `true` when an inbound Bearer was captured (used for env-fetch fallback).
    pub fn has_bearer(&self) -> bool {
        self.bearer_token.is_some()
    }

    fn transport(&self) -> Transport {
        Transport::new(
            self.platform_url.clone(),
            self.api_key.clone(),
            self.cookie_value.clone(),
            self.http.clone(),
        )
    }
}

fn resolve_platform_url(override_url: Option<String>) -> String {
    let raw = override_url
        .or_else(|| std::env::var("LEASH_PLATFORM_URL").ok())
        .unwrap_or_else(|| DEFAULT_PLATFORM_URL.to_string());
    let trimmed = raw.trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        DEFAULT_PLATFORM_URL.to_string()
    } else {
        trimmed
    }
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn extract_bearer(value: String) -> Option<String> {
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let tok = parts.next()?.trim();
    if tok.is_empty() {
        None
    } else {
        Some(tok.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req_with(cookie: Option<&str>, auth: Option<&str>) -> http::Request<()> {
        let mut builder = http::Request::builder().uri("/");
        if let Some(c) = cookie {
            builder = builder.header("cookie", c);
        }
        if let Some(a) = auth {
            builder = builder.header("authorization", a);
        }
        builder.body(()).unwrap()
    }

    #[test]
    fn new_captures_cookie() {
        std::env::remove_var("LEASH_API_KEY");
        let req = req_with(Some("leash-auth=tok"), None);
        let leash = Leash::new(&req).unwrap();
        assert!(leash.has_cookie());
        assert!(!leash.has_api_key());
        assert!(!leash.has_bearer());
    }

    #[test]
    fn new_captures_bearer() {
        std::env::remove_var("LEASH_API_KEY");
        let req = req_with(None, Some("Bearer abc"));
        let leash = Leash::new(&req).unwrap();
        assert!(leash.has_bearer());
        assert!(!leash.has_api_key());
    }

    #[test]
    fn from_api_key_rejects_empty() {
        let err = Leash::from_api_key("").unwrap_err();
        assert!(err.is_unauthorized());
    }

    #[test]
    fn from_token_rejects_empty() {
        let err = Leash::from_token("").unwrap_err();
        assert!(err.is_unauthorized());
    }

    #[test]
    fn with_api_key_overrides_env() {
        let req = req_with(None, None);
        let leash = Leash::new(&req).unwrap().with_api_key("override");
        assert!(leash.has_api_key());
    }

    #[test]
    fn with_platform_url_trims_trailing_slash() {
        let req = req_with(None, None);
        let leash = Leash::new(&req)
            .unwrap()
            .with_platform_url("https://staging.leash.build/");
        assert_eq!(leash.platform_url(), "https://staging.leash.build");
    }

    #[test]
    fn extract_bearer_handles_missing_and_malformed() {
        assert_eq!(extract_bearer("Bearer abc".to_string()), Some("abc".to_string()));
        assert_eq!(extract_bearer("bearer abc".to_string()), Some("abc".to_string()));
        assert_eq!(extract_bearer("Token abc".to_string()), None);
        assert_eq!(extract_bearer("Bearer  ".to_string()), None);
        assert_eq!(extract_bearer("".to_string()), None);
    }

    #[test]
    fn env_fallback_uses_bearer_when_no_api_key() {
        std::env::remove_var("LEASH_API_KEY");
        let req = req_with(None, Some("Bearer fallback_jwt"));
        let leash = Leash::new(&req).unwrap();
        // Construct env namespace and check the api_key bag (private) by
        // observing behaviour through has_bearer + has_api_key inspectors.
        assert!(leash.has_bearer());
        assert!(!leash.has_api_key());
        // The env namespace constructor folds bearer→key — exercised in HTTP tests.
        let _env = leash.env();
    }
}
