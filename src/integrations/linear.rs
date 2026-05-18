//! Linear provider.
//!
//! Wire action names are underscored (`list_issues`, etc.) — preserved here.
//! Tolerant of envelope vs. bare-array response shapes (the platform sometimes
//! returns `[...]`, sometimes `{ issues: [...] }`).

use serde::Serialize;
use serde_json::json;

use crate::errors::{LeashError, Result};
use crate::transport::Transport;

use super::types::{
    LinearComment, LinearCreateIssueInput, LinearIssue, LinearListIssuesFilter,
    LinearListIssuesResult, LinearListProjectsFilter, LinearProject, LinearTeam,
    LinearUpdateIssuePatch,
};

const PROVIDER: &str = "linear";

/// Typed Linear provider client.
#[derive(Debug, Clone)]
pub struct Linear {
    transport: Transport,
}

impl Linear {
    pub(crate) fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// List issues, optionally filtered. Tolerates envelope `{issues}` and bare `[]` responses.
    pub async fn list_issues(
        &self,
        filter: LinearListIssuesFilter,
    ) -> Result<LinearListIssuesResult> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list_issues", &filter)
            .await?;
        if raw.is_null() {
            return Ok(LinearListIssuesResult::default());
        }
        if raw.is_array() {
            let issues: Vec<LinearIssue> =
                serde_json::from_value(raw).map_err(|e| LeashError::MalformedResponse {
                    message: format!("Linear list_issues: failed to decode array: {e}"),
                })?;
            return Ok(LinearListIssuesResult {
                issues,
                cursor: None,
            });
        }
        serde_json::from_value(raw).map_err(|e| LeashError::MalformedResponse {
            message: format!("Linear list_issues: failed to decode envelope: {e}"),
        })
    }

    /// Get a single issue by Linear UUID.
    pub async fn get_issue(&self, id: &str) -> Result<LinearIssue> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "get_issue", &json!({ "id": id }))
            .await?;
        decode(raw, "get_issue")
    }

    /// Create a new Linear issue.
    pub async fn create_issue(&self, input: LinearCreateIssueInput) -> Result<LinearIssue> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "create_issue", &input)
            .await?;
        decode(raw, "create_issue")
    }

    /// Patch an issue. Only fields set on the patch are forwarded (`Option<T>` fields skipped when `None`).
    pub async fn update_issue(
        &self,
        id: &str,
        patch: LinearUpdateIssuePatch,
    ) -> Result<LinearIssue> {
        #[derive(Serialize)]
        struct Body<'a> {
            id: &'a str,
            #[serde(flatten)]
            patch: LinearUpdateIssuePatch,
        }
        let raw = self
            .transport
            .integrations_call(PROVIDER, "update_issue", &Body { id, patch })
            .await?;
        decode(raw, "update_issue")
    }

    /// Post a comment to an existing issue.
    pub async fn add_comment(&self, issue_id: &str, body: &str) -> Result<LinearComment> {
        let raw = self
            .transport
            .integrations_call(
                PROVIDER,
                "add_comment",
                &json!({ "issueId": issue_id, "body": body }),
            )
            .await?;
        decode(raw, "add_comment")
    }

    /// List all teams visible to the authenticated user.
    pub async fn list_teams(&self) -> Result<Vec<LinearTeam>> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list_teams", &json!({}))
            .await?;
        if raw.is_null() {
            return Ok(vec![]);
        }
        if raw.is_array() {
            return serde_json::from_value(raw).map_err(|e| LeashError::MalformedResponse {
                message: format!("Linear list_teams: failed to decode array: {e}"),
            });
        }
        let envelope: serde_json::Value = raw;
        if let Some(teams) = envelope.get("teams").cloned() {
            return serde_json::from_value(teams).map_err(|e| LeashError::MalformedResponse {
                message: format!("Linear list_teams: failed to decode teams: {e}"),
            });
        }
        Ok(vec![])
    }

    /// List Linear projects, optionally filtered by team.
    pub async fn list_projects(
        &self,
        filter: LinearListProjectsFilter,
    ) -> Result<Vec<LinearProject>> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list_projects", &filter)
            .await?;
        if raw.is_null() {
            return Ok(vec![]);
        }
        if raw.is_array() {
            return serde_json::from_value(raw).map_err(|e| LeashError::MalformedResponse {
                message: format!("Linear list_projects: failed to decode array: {e}"),
            });
        }
        let envelope: serde_json::Value = raw;
        if let Some(projects) = envelope.get("projects").cloned() {
            return serde_json::from_value(projects).map_err(|e| LeashError::MalformedResponse {
                message: format!("Linear list_projects: failed to decode projects: {e}"),
            });
        }
        Ok(vec![])
    }
}

fn decode<T: serde::de::DeserializeOwned + Default>(
    raw: serde_json::Value,
    action: &str,
) -> Result<T> {
    if raw.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(raw).map_err(|e| LeashError::MalformedResponse {
        message: format!("Linear {action}: failed to decode: {e}"),
    })
}
