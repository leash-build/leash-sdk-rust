//! Wire types for integration responses. Mirrors the shapes in
//! `leash-sdk-ts/src/integrations/types.ts` and the Linear types in
//! `leash-sdk-ts/src/integrations/providers/linear.ts`.
//!
//! Field names and JSON renames follow the platform wire format (camelCase),
//! so `serde_json` round-trips through these types are zero-effort.

#![allow(missing_docs)]


use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Gmail
// ---------------------------------------------------------------------------

/// A single Gmail message reference.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GmailMessage {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "threadId", default)]
    pub thread_id: String,
}

/// Paginated Gmail message list.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GmailMessageList {
    #[serde(default)]
    pub messages: Vec<GmailMessage>,
    #[serde(rename = "nextPageToken", default, skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate", default, skip_serializing_if = "Option::is_none")]
    pub result_size_estimate: Option<u64>,
}

/// A single Gmail label.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GmailLabel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub label_type: String,
}

/// Gmail label list.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GmailLabelList {
    #[serde(default)]
    pub labels: Vec<GmailLabel>,
}

/// Parameters for `gmail().list_messages()`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct GmailListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(rename = "maxResults", skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(rename = "labelIds", skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
    #[serde(rename = "pageToken", skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
}

/// Parameters for `gmail().send_message()`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct GmailSendMessageParams {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc: Option<String>,
}

/// `format` selector for `gmail().get_message()`.
#[derive(Debug, Clone, Copy)]
pub enum GmailMessageFormat {
    Full,
    Metadata,
    Minimal,
    Raw,
}

impl GmailMessageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Metadata => "metadata",
            Self::Minimal => "minimal",
            Self::Raw => "raw",
        }
    }
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

/// Entry in the user's calendar list.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarListEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(rename = "timeZone", default, skip_serializing_if = "String::is_empty")]
    pub time_zone: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub primary: bool,
    #[serde(rename = "backgroundColor", default, skip_serializing_if = "String::is_empty")]
    pub background_color: String,
    #[serde(rename = "foregroundColor", default, skip_serializing_if = "String::is_empty")]
    pub foreground_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarList {
    #[serde(default)]
    pub calendars: Vec<CalendarListEntry>,
}

/// Google-style date/time descriptor. Exactly one of `date_time` (timed) or
/// `date` (all-day) should be populated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDateTime {
    #[serde(rename = "dateTime", default, skip_serializing_if = "Option::is_none")]
    pub date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(rename = "timeZone", default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarAttendee {
    pub email: String,
    #[serde(rename = "responseStatus", default, skip_serializing_if = "Option::is_none")]
    pub response_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarEvent {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    #[serde(default)]
    pub start: EventDateTime,
    #[serde(default)]
    pub end: EventDateTime,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<CalendarAttendee>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(rename = "htmlLink", default, skip_serializing_if = "String::is_empty")]
    pub html_link: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarEventList {
    #[serde(default)]
    pub events: Vec<CalendarEvent>,
    #[serde(rename = "nextPageToken", default, skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CalendarListEventsParams {
    #[serde(rename = "calendarId", skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    #[serde(rename = "timeMin", skip_serializing_if = "Option::is_none")]
    pub time_min: Option<String>,
    #[serde(rename = "timeMax", skip_serializing_if = "Option::is_none")]
    pub time_max: Option<String>,
    #[serde(rename = "maxResults", skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(rename = "singleEvents", skip_serializing_if = "Option::is_none")]
    pub single_events: Option<bool>,
    #[serde(rename = "orderBy", skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CalendarCreateEventParams {
    #[serde(rename = "calendarId", skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub start: EventDateTime,
    pub end: EventDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendees: Option<Vec<CalendarAttendee>>,
}

// ---------------------------------------------------------------------------
// Drive
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriveFile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(rename = "createdTime", default, skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(rename = "modifiedTime", default, skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<String>,
    #[serde(rename = "webViewLink", default, skip_serializing_if = "Option::is_none")]
    pub web_view_link: Option<String>,
    #[serde(rename = "webContentLink", default, skip_serializing_if = "Option::is_none")]
    pub web_content_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriveFileList {
    #[serde(default)]
    pub files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken", default, skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DriveListFilesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(rename = "maxResults", skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(rename = "folderId", skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DriveUploadFileParams {
    pub name: String,
    pub content: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Linear
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LinearUserRef {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub email: String,
    #[serde(rename = "displayName", default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearStateRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub state_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearTeamRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearIssue {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub identifier: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(rename = "createdAt", default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    #[serde(rename = "updatedAt", default, skip_serializing_if = "String::is_empty")]
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<LinearUserRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<LinearStateRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<LinearTeamRef>,
    #[serde(rename = "labelIds", default, skip_serializing_if = "Vec::is_empty")]
    pub label_ids: Vec<String>,
    #[serde(rename = "projectId", default, skip_serializing_if = "String::is_empty")]
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearComment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub body: String,
    #[serde(rename = "issueId", default)]
    pub issue_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<LinearUserRef>,
    #[serde(rename = "createdAt", default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    #[serde(rename = "updatedAt", default, skip_serializing_if = "String::is_empty")]
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearTeam {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub private: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearProject {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    #[serde(rename = "targetDate", default, skip_serializing_if = "String::is_empty")]
    pub target_date: String,
    #[serde(rename = "startDate", default, skip_serializing_if = "String::is_empty")]
    pub start_date: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(rename = "teamIds", default, skip_serializing_if = "Vec::is_empty")]
    pub team_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LinearListIssuesFilter {
    #[serde(rename = "teamId", skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(rename = "assigneeId", skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,
    #[serde(rename = "stateType", skip_serializing_if = "Option::is_none")]
    pub state_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinearListIssuesResult {
    #[serde(default)]
    pub issues: Vec<LinearIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LinearCreateIssueInput {
    #[serde(rename = "teamId")]
    pub team_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "assigneeId", skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(rename = "labelIds", skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LinearUpdateIssuePatch {
    #[serde(rename = "teamId", skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "assigneeId", skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(rename = "labelIds", skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LinearListProjectsFilter {
    #[serde(rename = "teamId", skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Shared utility — connection status (kept for compatibility w/ env-fetch test paths)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionStatus {
    #[serde(rename = "providerId", default)]
    pub provider_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "expiresAt", default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Header bag alias used by the custom MCP server config shape.
pub type Headers = HashMap<String, String>;
