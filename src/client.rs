use std::collections::HashMap;
use std::sync::Mutex;

use crate::calendar::CalendarClient;
use crate::custom::CustomIntegration;
use crate::drive::DriveClient;
use crate::gmail::GmailClient;
use crate::types::{ApiResponse, ConnectionStatus, LeashError, DEFAULT_PLATFORM_URL};

/// Main client for the Leash platform integrations API.
///
/// Create one with [`LeashIntegrations::new`] then access provider clients
/// via [`gmail()`](Self::gmail), [`calendar()`](Self::calendar), and
/// [`drive()`](Self::drive).
///
/// # Example
/// ```no_run
/// # async fn example() -> Result<(), leash_sdk::LeashError> {
/// use leash_sdk::LeashIntegrations;
///
/// let client = LeashIntegrations::new("my-jwt-token");
/// let messages = client.gmail().list_messages(None).await?;
/// # Ok(())
/// # }
/// ```
pub struct LeashIntegrations {
    pub(crate) platform_url: String,
    pub(crate) auth_token: String,
    pub(crate) api_key: Option<String>,
    pub(crate) http: reqwest::Client,
    env_cache: Mutex<Option<HashMap<String, String>>>,
}

impl LeashIntegrations {
    /// Create a new client with the given auth token and the default platform URL.
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            platform_url: DEFAULT_PLATFORM_URL.to_string(),
            auth_token: auth_token.into(),
            api_key: std::env::var("LEASH_API_KEY").ok(),
            http: reqwest::Client::new(),
            env_cache: Mutex::new(None),
        }
    }

    /// Set a custom platform URL (overrides the default `https://leash.build`).
    pub fn with_platform_url(mut self, url: impl Into<String>) -> Self {
        self.platform_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Set an API key for service-to-service authentication.
    ///
    /// When set, the key is sent as the `X-API-Key` header on every request.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set a custom reqwest HTTP client.
    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http = client;
        self
    }

    /// Return a [`GmailClient`] for interacting with the Gmail integration.
    pub fn gmail(&self) -> GmailClient<'_> {
        GmailClient { client: self }
    }

    /// Return a [`CalendarClient`] for interacting with the Google Calendar integration.
    pub fn calendar(&self) -> CalendarClient<'_> {
        CalendarClient { client: self }
    }

    /// Return a [`DriveClient`] for interacting with the Google Drive integration.
    pub fn drive(&self) -> DriveClient<'_> {
        DriveClient { client: self }
    }

    /// Return a [`CustomIntegration`] for the given integration name.
    ///
    /// This is the escape hatch for custom or untyped integrations that don't
    /// have dedicated provider clients.
    pub fn integration(&self, name: &str) -> CustomIntegration<'_> {
        CustomIntegration::new(name, self)
    }

    /// Perform a generic integration API call.
    ///
    /// Sends `POST {platform_url}/api/integrations/{provider}/{action}` with the
    /// given JSON body and returns the `data` field from the response envelope.
    pub async fn call(
        &self,
        provider: &str,
        action: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, LeashError> {
        self.call_internal(provider, action, body).await
    }

    /// Internal call used by all provider clients.
    pub(crate) async fn call_internal(
        &self,
        provider: &str,
        action: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, LeashError> {
        let url = format!(
            "{}/api/integrations/{}/{}",
            self.platform_url, provider, action
        );

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .bearer_auth(&self.auth_token);

        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req.send().await?;
        let api_resp: ApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(api_resp.into_error());
        }

        Ok(api_resp.data.unwrap_or(serde_json::Value::Null))
    }

    /// Check whether a provider is connected for the current user.
    pub async fn is_connected(&self, provider_id: &str) -> bool {
        match self.get_connections().await {
            Ok(connections) => connections
                .iter()
                .any(|c| c.provider_id == provider_id && c.status == "active"),
            Err(_) => false,
        }
    }

    /// Get connection status for all providers.
    pub async fn get_connections(&self) -> Result<Vec<ConnectionStatus>, LeashError> {
        let url = format!("{}/api/integrations/connections", self.platform_url);

        let mut req = self.http.get(&url);

        if !self.auth_token.is_empty() {
            req = req.bearer_auth(&self.auth_token);
        }
        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await?;
        let api_resp: ApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(api_resp.into_error());
        }

        let data = api_resp.data.unwrap_or(serde_json::Value::Null);
        let connections: Vec<ConnectionStatus> =
            serde_json::from_value(data).map_err(|e| LeashError::ApiError {
                message: format!("failed to parse connections: {e}"),
                code: None,
            })?;

        Ok(connections)
    }

    /// Call any MCP server tool directly via the Leash platform.
    ///
    /// Sends `POST {platform_url}/api/mcp/run` with the given npm package name,
    /// tool name, and optional arguments, then returns the `data` field.
    pub async fn mcp(
        &self,
        package: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, LeashError> {
        let url = format!("{}/api/mcp/run", self.platform_url);

        let payload = serde_json::json!({
            "package": package,
            "tool": tool,
            "args": args,
        });

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload);

        if !self.auth_token.is_empty() {
            req = req.bearer_auth(&self.auth_token);
        }
        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await?;
        let api_resp: ApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(api_resp.into_error());
        }

        Ok(api_resp.data.unwrap_or(serde_json::Value::Null))
    }

    /// Fetch all environment variables from the Leash platform.
    ///
    /// The result is cached after the first successful call.
    pub async fn get_env(&self) -> Result<HashMap<String, String>, LeashError> {
        // Check cache first.
        {
            let cache = self.env_cache.lock().unwrap();
            if let Some(ref cached) = *cache {
                return Ok(cached.clone());
            }
        }

        let url = format!("{}/api/apps/env", self.platform_url);

        let mut req = self.http.get(&url);

        if !self.auth_token.is_empty() {
            req = req.bearer_auth(&self.auth_token);
        }
        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }

        let resp = req.send().await?;
        let api_resp: ApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(api_resp.into_error());
        }

        let data = api_resp.data.unwrap_or(serde_json::Value::Null);
        let env_map: HashMap<String, String> =
            serde_json::from_value(data).map_err(|e| LeashError::ApiError {
                message: format!("failed to parse env data: {e}"),
                code: None,
            })?;

        // Store in cache.
        {
            let mut cache = self.env_cache.lock().unwrap();
            *cache = Some(env_map.clone());
        }

        Ok(env_map)
    }

    /// Fetch a single environment variable by key.
    ///
    /// Returns `None` if the key is not present.
    pub async fn get_env_key(&self, key: &str) -> Result<Option<String>, LeashError> {
        let env_map = self.get_env().await?;
        Ok(env_map.get(key).cloned())
    }

    /// Get the URL to initiate an OAuth connection flow for the given provider.
    ///
    /// Use this URL in UI buttons or redirects to connect a user's account.
    pub fn get_connect_url(&self, provider_id: &str, return_url: Option<&str>) -> String {
        let base = format!(
            "{}/api/integrations/connect/{}",
            self.platform_url, provider_id
        );
        match return_url {
            Some(url) => {
                let encoded = urlencoding_encode(url);
                format!("{base}?return_url={encoded}")
            }
            None => base,
        }
    }
}

/// Minimal percent-encoding for the return_url query parameter.
fn urlencoding_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
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
