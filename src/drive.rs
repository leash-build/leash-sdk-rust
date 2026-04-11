use crate::client::LeashIntegrations;
use crate::types::{LeashError, ListFilesParams};

const PROVIDER: &str = "google_drive";

/// Client for the Google Drive integration.
///
/// Obtained via [`LeashIntegrations::drive()`].
pub struct DriveClient<'a> {
    pub(crate) client: &'a LeashIntegrations,
}

impl<'a> DriveClient<'a> {
    /// List files in the user's Drive.
    ///
    /// Pass `None` to use server defaults.
    pub async fn list_files(
        &self,
        params: Option<ListFilesParams>,
    ) -> Result<serde_json::Value, LeashError> {
        let body = params.map(|p| serde_json::to_value(p).unwrap());
        self.client.call_internal(PROVIDER, "list-files", body).await
    }

    /// Get file metadata by ID.
    pub async fn get_file(
        &self,
        file_id: &str,
    ) -> Result<serde_json::Value, LeashError> {
        let body = serde_json::json!({ "fileId": file_id });
        self.client.call_internal(PROVIDER, "get-file", Some(body)).await
    }

    /// Search files using a query string.
    pub async fn search_files(
        &self,
        query: &str,
        max_results: Option<u32>,
    ) -> Result<serde_json::Value, LeashError> {
        let mut body = serde_json::json!({ "query": query });
        if let Some(max) = max_results {
            body["maxResults"] = serde_json::Value::Number(max.into());
        }
        self.client.call_internal(PROVIDER, "search-files", Some(body)).await
    }
}
