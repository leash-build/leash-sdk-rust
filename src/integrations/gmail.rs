//! Gmail provider — mirrors `leash.integrations.gmail` in the TS / Python SDKs.

use serde::Serialize;
use serde_json::json;

use crate::errors::Result;
use crate::transport::Transport;

use super::types::{
    GmailLabelList, GmailListParams, GmailMessageFormat, GmailMessageList, GmailSendMessageParams,
};

const PROVIDER: &str = "gmail";

/// Typed Gmail provider client.
#[derive(Debug, Clone)]
pub struct Gmail {
    transport: Transport,
}

impl Gmail {
    pub(crate) fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// List messages in the user's mailbox.
    pub async fn list_messages(&self, params: GmailListParams) -> Result<GmailMessageList> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list-messages", &params)
            .await?;
        decode(raw)
    }

    /// Get a single message by ID.
    pub async fn get_message(
        &self,
        message_id: &str,
        format: Option<GmailMessageFormat>,
    ) -> Result<serde_json::Value> {
        let body = if let Some(fmt) = format {
            json!({ "messageId": message_id, "format": fmt.as_str() })
        } else {
            json!({ "messageId": message_id })
        };
        self.transport
            .integrations_call(PROVIDER, "get-message", &body)
            .await
    }

    /// Send an email message.
    pub async fn send_message(
        &self,
        params: GmailSendMessageParams,
    ) -> Result<serde_json::Value> {
        self.transport
            .integrations_call(PROVIDER, "send-message", &params)
            .await
    }

    /// Run a Gmail search query.
    pub async fn search_messages(
        &self,
        query: &str,
        max_results: Option<u32>,
    ) -> Result<GmailMessageList> {
        #[derive(Serialize)]
        struct Body<'a> {
            query: &'a str,
            #[serde(rename = "maxResults", skip_serializing_if = "Option::is_none")]
            max_results: Option<u32>,
        }
        let raw = self
            .transport
            .integrations_call(
                PROVIDER,
                "search-messages",
                &Body { query, max_results },
            )
            .await?;
        decode(raw)
    }

    /// List all labels in the user's mailbox.
    pub async fn list_labels(&self) -> Result<GmailLabelList> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list-labels", &json!({}))
            .await?;
        decode(raw)
    }

    /// Get the authenticated user's Gmail profile.
    pub async fn get_profile(&self) -> Result<serde_json::Value> {
        self.transport
            .integrations_call(PROVIDER, "get-profile", &json!({}))
            .await
    }
}

fn decode<T: serde::de::DeserializeOwned + Default>(raw: serde_json::Value) -> Result<T> {
    if raw.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(raw).map_err(|e| crate::errors::LeashError::MalformedResponse {
        message: format!("Failed to deserialise Gmail response: {e}"),
    })
}
