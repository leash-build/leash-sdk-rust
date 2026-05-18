//! Shared HTTP transport for integration POSTs.
//!
//! Mirrors the TS `_call` / Python `_Transport.call` / Go `Transport.post`.
//!
//! Critical platform contract (see `leash-sdk-ts/src/leash.ts` lines 586–601):
//!
//! - Only `X-API-Key` and `Cookie` are forwarded on integration calls.
//! - `Authorization: Bearer …` is intentionally **NOT** forwarded — the
//!   platform's `verifyToken()` can reject a user JWT before the API-key check
//!   runs, breaking the call. Bearer is still captured by [`crate::Leash`]
//!   for env-fetch fallback, but never sent here.

use std::sync::Arc;

use serde::Serialize;

use crate::errors::{LeashError, Result};

#[derive(Debug, Clone)]
pub(crate) struct Transport {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    platform_url: String,
    api_key: Option<String>,
    cookie_value: Option<String>,
    http: reqwest::Client,
}

impl Transport {
    pub(crate) fn new(
        platform_url: String,
        api_key: Option<String>,
        cookie_value: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                platform_url: platform_url.trim_end_matches('/').to_string(),
                api_key,
                cookie_value,
                http,
            }),
        }
    }

    /// POST to `/api/integrations/{provider}/{action}` and unwrap the
    /// `{ success, data }` envelope.
    pub(crate) async fn integrations_call<B>(
        &self,
        provider: &str,
        action: &str,
        body: &B,
    ) -> Result<serde_json::Value>
    where
        B: Serialize + ?Sized,
    {
        let url = format!(
            "{}/api/integrations/{}/{}",
            self.inner.platform_url, provider, action
        );
        let docs_url = format!("https://leash.build/docs/integrations/{provider}");
        self.post(&url, body, provider, &docs_url).await
    }

    async fn post<B>(
        &self,
        url: &str,
        body: &B,
        provider: &str,
        docs_url: &str,
    ) -> Result<serde_json::Value>
    where
        B: Serialize + ?Sized,
    {
        let mut req = self.inner.http.post(url).header("Content-Type", "application/json");

        // Critical: X-API-Key + Cookie only. Bearer is intentionally not
        // forwarded — see module docstring.
        if let Some(ref key) = self.inner.api_key {
            req = req.header("X-API-Key", key);
        }
        if let Some(ref cookie) = self.inner.cookie_value {
            req = req.header("Cookie", format!("leash-auth={cookie}"));
        }

        let resp = req.json(body).send().await?;
        let status = resp.status();
        let raw = resp.bytes().await?;

        if !status.is_success() {
            return Err(map_integration_error(
                status.as_u16(),
                &raw,
                provider,
                docs_url,
            ));
        }

        // Body may be `{"success":true,"data":...}` or `{"data":...}` or a
        // bare shape (Linear sometimes returns arrays directly).
        if raw.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| LeashError::MalformedResponse {
                message: format!("Failed to parse platform response as JSON: {e}"),
            })?;

        if let serde_json::Value::Object(ref map) = value {
            // Surface envelope-level failures (some 2xx responses carry success=false).
            if let Some(serde_json::Value::Bool(false)) = map.get("success") {
                let msg = map
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Integration error")
                    .to_string();
                let code = map.get("code").and_then(|v| v.as_str()).unwrap_or("");
                return Err(match code {
                    "UPGRADE_REQUIRED" => LeashError::PlanBlock {
                        code: code.into(),
                        message: msg,
                        required_plan: map
                            .get("requiredPlan")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    },
                    "UNAUTHORIZED" => LeashError::Unauthorized { message: msg },
                    _ => LeashError::UpstreamError {
                        status: status.as_u16(),
                        message: msg,
                    },
                });
            }
            if let Some(data) = map.get("data") {
                return Ok(data.clone());
            }
        }
        Ok(value)
    }
}

fn map_integration_error(status: u16, raw: &[u8], provider: &str, docs_url: &str) -> LeashError {
    // Best-effort message + connect_url extraction.
    let parsed: Option<serde_json::Value> = serde_json::from_slice(raw).ok();
    let message = parsed
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("HTTP {status}"));

    match status {
        401 => LeashError::Unauthorized { message },
        402 => {
            let msg = parsed
                .as_ref()
                .and_then(|v| v.get("message"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "This feature requires a higher plan.".to_string());
            let required_plan = parsed
                .as_ref()
                .and_then(|v| v.get("requiredPlan"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            LeashError::PlanBlock {
                code: "UPGRADE_REQUIRED".into(),
                message: msg,
                required_plan,
            }
        }
        403 => {
            let connect_url = parsed
                .as_ref()
                .and_then(|v| v.get("connectUrl"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            LeashError::ConnectionRequired {
                provider: provider.to_string(),
                message,
                connect_url,
            }
        }
        _ => {
            // Mention the docs URL so the upstream error is actionable
            // without the caller having to look it up.
            let _ = docs_url; // currently we keep docs_url out of the message body but kept for parity
            LeashError::UpstreamError { status, message }
        }
    }
}
