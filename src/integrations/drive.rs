//! Google Drive provider.

use serde::Serialize;
use serde_json::json;

use crate::errors::Result;
use crate::transport::Transport;

use super::types::{DriveFile, DriveFileList, DriveListFilesParams, DriveUploadFileParams};

const PROVIDER: &str = "google_drive";

/// Typed Google Drive provider client. Obtain via
/// `leash.integrations().drive()` or `.google_drive()`.
#[derive(Debug, Clone)]
pub struct Drive {
    transport: Transport,
}

impl Drive {
    pub(crate) fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// List files from the user's Drive. Pass `Default::default()` for platform defaults.
    pub async fn list_files(&self, params: DriveListFilesParams) -> Result<DriveFileList> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list-files", &params)
            .await?;
        decode(raw)
    }

    /// Retrieve a file's metadata by ID.
    pub async fn get_file(&self, file_id: &str) -> Result<DriveFile> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "get-file", &json!({ "fileId": file_id }))
            .await?;
        decode(raw)
    }

    /// Download a file's content envelope (raw JSON — base64 bytes or text depending on file type).
    pub async fn download_file(&self, file_id: &str) -> Result<serde_json::Value> {
        self.transport
            .integrations_call(PROVIDER, "download-file", &json!({ "fileId": file_id }))
            .await
    }

    /// Create a new folder. Pass `None` for `parent_id` to create at the root of My Drive.
    pub async fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<DriveFile> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
            parent_id: Option<&'a str>,
        }
        let raw = self
            .transport
            .integrations_call(PROVIDER, "create-folder", &Body { name, parent_id })
            .await?;
        decode(raw)
    }

    /// Upload a new file with the given content.
    pub async fn upload_file(&self, params: DriveUploadFileParams) -> Result<DriveFile> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "upload-file", &params)
            .await?;
        decode(raw)
    }

    /// Permanently delete a file by ID.
    pub async fn delete_file(&self, file_id: &str) -> Result<serde_json::Value> {
        self.transport
            .integrations_call(PROVIDER, "delete-file", &json!({ "fileId": file_id }))
            .await
    }

    /// Run a Drive-syntax search query. Pass `None` for `max_results` to use the platform default.
    pub async fn search_files(
        &self,
        query: &str,
        max_results: Option<u32>,
    ) -> Result<DriveFileList> {
        #[derive(Serialize)]
        struct Body<'a> {
            query: &'a str,
            #[serde(rename = "maxResults", skip_serializing_if = "Option::is_none")]
            max_results: Option<u32>,
        }
        let raw = self
            .transport
            .integrations_call(PROVIDER, "search-files", &Body { query, max_results })
            .await?;
        decode(raw)
    }
}

fn decode<T: serde::de::DeserializeOwned + Default>(raw: serde_json::Value) -> Result<T> {
    if raw.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(raw).map_err(|e| crate::errors::LeashError::MalformedResponse {
        message: format!("Failed to deserialise Drive response: {e}"),
    })
}
