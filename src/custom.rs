use crate::client::LeashIntegrations;
use crate::types::{ApiResponse, LeashError};
use serde::Serialize;

/// Untyped client for a custom integration.
///
/// Obtained via [`LeashIntegrations::integration`]. Proxies requests through
/// the Leash platform at `/api/integrations/custom/{name}`.
///
/// # Example
/// ```no_run
/// # async fn example() -> Result<(), leash_sdk::LeashError> {
/// use leash_sdk::LeashIntegrations;
///
/// let client = LeashIntegrations::new("my-jwt-token");
/// let stripe = client.integration("stripe");
/// let charges = stripe.call("/v1/charges", "GET", None).await?;
/// # Ok(())
/// # }
/// ```
pub struct CustomIntegration<'a> {
    name: String,
    client: &'a LeashIntegrations,
}

#[derive(Serialize)]
struct CustomCallRequest {
    path: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<std::collections::HashMap<String, String>>,
}

impl<'a> CustomIntegration<'a> {
    pub(crate) fn new(name: &str, client: &'a LeashIntegrations) -> Self {
        Self {
            name: name.to_string(),
            client,
        }
    }

    /// Invoke the custom integration proxy.
    ///
    /// Sends a POST to `/api/integrations/custom/{name}` with the given path,
    /// method, and optional body forwarded to the upstream service.
    pub async fn call(
        &self,
        path: &str,
        method: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, LeashError> {
        self.call_with_headers(path, method, body, None).await
    }

    /// Like [`call`](Self::call) but also forwards custom headers.
    pub async fn call_with_headers(
        &self,
        path: &str,
        method: &str,
        body: Option<serde_json::Value>,
        headers: Option<std::collections::HashMap<String, String>>,
    ) -> Result<serde_json::Value, LeashError> {
        let url = format!(
            "{}/api/integrations/custom/{}",
            self.client.platform_url, self.name
        );

        let payload = CustomCallRequest {
            path: path.to_string(),
            method: method.to_string(),
            body,
            headers,
        };

        let mut req = self
            .client
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .bearer_auth(&self.client.auth_token)
            .json(&payload);

        if let Some(ref key) = self.client.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await?;
        let api_resp: ApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(api_resp.into_error());
        }

        Ok(api_resp.data.unwrap_or(serde_json::Value::Null))
    }
}
