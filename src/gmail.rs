use crate::client::LeashIntegrations;
use crate::types::{LeashError, ListMessagesParams, SendMessageParams};

const PROVIDER: &str = "gmail";

/// Client for the Gmail integration.
///
/// Obtained via [`LeashIntegrations::gmail()`].
pub struct GmailClient<'a> {
    pub(crate) client: &'a LeashIntegrations,
}

impl<'a> GmailClient<'a> {
    /// List messages in the user's mailbox.
    ///
    /// Pass `None` to use server defaults.
    pub async fn list_messages(
        &self,
        params: Option<ListMessagesParams>,
    ) -> Result<serde_json::Value, LeashError> {
        let body = params.map(|p| serde_json::to_value(p).unwrap());
        self.client.call_internal(PROVIDER, "list-messages", body).await
    }

    /// Get a single message by ID.
    ///
    /// `format` controls the response detail: `"full"`, `"metadata"`, `"minimal"`, or `"raw"`.
    /// Pass `None` for the server default (`"full"`).
    pub async fn get_message(
        &self,
        message_id: &str,
        format: Option<&str>,
    ) -> Result<serde_json::Value, LeashError> {
        let mut body = serde_json::json!({ "messageId": message_id });
        if let Some(fmt) = format {
            body["format"] = serde_json::Value::String(fmt.to_string());
        }
        self.client.call_internal(PROVIDER, "get-message", Some(body)).await
    }

    /// Send an email message.
    pub async fn send_message(
        &self,
        params: SendMessageParams,
    ) -> Result<serde_json::Value, LeashError> {
        let body = serde_json::to_value(params).unwrap();
        self.client.call_internal(PROVIDER, "send-message", Some(body)).await
    }

    /// Search messages using a Gmail query string.
    pub async fn search_messages(
        &self,
        query: &str,
        max_results: Option<u32>,
    ) -> Result<serde_json::Value, LeashError> {
        let mut body = serde_json::json!({ "query": query });
        if let Some(max) = max_results {
            body["maxResults"] = serde_json::Value::Number(max.into());
        }
        self.client.call_internal(PROVIDER, "search-messages", Some(body)).await
    }

    /// List all labels in the user's mailbox.
    pub async fn list_labels(&self) -> Result<serde_json::Value, LeashError> {
        self.client.call_internal(PROVIDER, "list-labels", None).await
    }
}
