use serde::{Deserialize, Serialize};
use std::fmt;

/// Default Leash platform URL.
pub const DEFAULT_PLATFORM_URL: &str = "https://leash.build";

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors returned by the Leash SDK.
#[derive(Debug)]
pub enum LeashError {
    /// The provider integration is not connected for the current user.
    NotConnected {
        message: String,
        connect_url: Option<String>,
    },
    /// The OAuth token has expired and needs to be refreshed.
    TokenExpired { message: String },
    /// The API returned an error response.
    ApiError {
        message: String,
        code: Option<String>,
    },
    /// A network or HTTP-level error occurred.
    NetworkError(reqwest::Error),
}

impl fmt::Display for LeashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeashError::NotConnected { message, .. } => write!(f, "leash: not connected: {message}"),
            LeashError::TokenExpired { message } => write!(f, "leash: token expired: {message}"),
            LeashError::ApiError { message, code } => {
                if let Some(c) = code {
                    write!(f, "leash: {message} (code: {c})")
                } else {
                    write!(f, "leash: {message}")
                }
            }
            LeashError::NetworkError(e) => write!(f, "leash: network error: {e}"),
        }
    }
}

impl std::error::Error for LeashError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LeashError::NetworkError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for LeashError {
    fn from(err: reqwest::Error) -> Self {
        LeashError::NetworkError(err)
    }
}

// ---------------------------------------------------------------------------
// API response envelope
// ---------------------------------------------------------------------------

/// The standard JSON envelope returned by all Leash platform API endpoints.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub code: Option<String>,
    #[serde(rename = "connectUrl")]
    pub connect_url: Option<String>,
}

impl ApiResponse {
    /// Convert a failed API response into the appropriate `LeashError`.
    pub fn into_error(self) -> LeashError {
        let message = self.error.unwrap_or_else(|| "unknown error".to_string());
        let code = self.code;

        match code.as_deref() {
            Some("not_connected") => LeashError::NotConnected {
                message,
                connect_url: self.connect_url,
            },
            Some("token_expired") => LeashError::TokenExpired { message },
            _ => LeashError::ApiError { message, code },
        }
    }
}

// ---------------------------------------------------------------------------
// Connection types
// ---------------------------------------------------------------------------

/// Status of a provider connection for the current user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    /// The provider identifier (e.g. "gmail", "google_calendar").
    #[serde(rename = "providerId")]
    pub provider_id: String,
    /// Connection status: "active", "expired", etc.
    pub status: String,
    /// The email associated with the connection, if available.
    #[serde(default)]
    pub email: Option<String>,
    /// When the OAuth token expires, if available.
    #[serde(default, rename = "expiresAt")]
    pub expires_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Gmail types
// ---------------------------------------------------------------------------

/// Parameters for listing Gmail messages.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ListMessagesParams {
    /// Gmail search query (e.g. "from:user@example.com").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Maximum number of messages to return.
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxResults")]
    pub max_results: Option<u32>,
    /// Filter messages by label IDs (e.g. ["INBOX"]).
    #[serde(skip_serializing_if = "Option::is_none", rename = "labelIds")]
    pub label_ids: Option<Vec<String>>,
    /// Token for fetching the next page of results.
    #[serde(skip_serializing_if = "Option::is_none", rename = "pageToken")]
    pub page_token: Option<String>,
}

/// Parameters for sending a Gmail message.
#[derive(Debug, Clone, Serialize)]
pub struct SendMessageParams {
    /// Recipient email address.
    pub to: String,
    /// Email subject line.
    pub subject: String,
    /// Email body text.
    pub body: String,
    /// Optional CC recipient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
    /// Optional BCC recipient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc: Option<String>,
}

// ---------------------------------------------------------------------------
// Calendar types
// ---------------------------------------------------------------------------

/// Parameters for listing calendar events.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ListEventsParams {
    /// Calendar identifier (defaults to "primary" on the server).
    #[serde(skip_serializing_if = "Option::is_none", rename = "calendarId")]
    pub calendar_id: Option<String>,
    /// Lower bound for event start time (RFC 3339).
    #[serde(skip_serializing_if = "Option::is_none", rename = "timeMin")]
    pub time_min: Option<String>,
    /// Upper bound for event start time (RFC 3339).
    #[serde(skip_serializing_if = "Option::is_none", rename = "timeMax")]
    pub time_max: Option<String>,
    /// Maximum number of events to return.
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxResults")]
    pub max_results: Option<u32>,
    /// Search query to filter events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Whether to expand recurring events into individual instances.
    #[serde(skip_serializing_if = "Option::is_none", rename = "singleEvents")]
    pub single_events: Option<bool>,
    /// Order of results (e.g. "startTime", "updated").
    #[serde(skip_serializing_if = "Option::is_none", rename = "orderBy")]
    pub order_by: Option<String>,
}

/// A start or end time for a calendar event.
#[derive(Debug, Default, Clone, Serialize)]
pub struct EventDateTime {
    /// RFC 3339 timestamp (for timed events).
    #[serde(skip_serializing_if = "Option::is_none", rename = "dateTime")]
    pub date_time: Option<String>,
    /// Date string for all-day events (format: "2006-01-02").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// IANA time zone (e.g. "America/New_York").
    #[serde(skip_serializing_if = "Option::is_none", rename = "timeZone")]
    pub time_zone: Option<String>,
}

/// A calendar event attendee.
#[derive(Debug, Clone, Serialize)]
pub struct Attendee {
    pub email: String,
}

/// Parameters for creating a calendar event.
#[derive(Debug, Clone, Serialize)]
pub struct CreateEventParams {
    /// Calendar identifier (defaults to "primary" on the server).
    #[serde(skip_serializing_if = "Option::is_none", rename = "calendarId")]
    pub calendar_id: Option<String>,
    /// Event title.
    pub summary: String,
    /// Optional event description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional event location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Event start time.
    pub start: EventDateTime,
    /// Event end time.
    pub end: EventDateTime,
    /// Optional list of attendees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attendees: Option<Vec<Attendee>>,
}

// ---------------------------------------------------------------------------
// Drive types
// ---------------------------------------------------------------------------

/// Parameters for listing Drive files.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ListFilesParams {
    /// Google Drive search query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Maximum number of files to return.
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxResults")]
    pub max_results: Option<u32>,
    /// Restrict results to files within a specific folder.
    #[serde(skip_serializing_if = "Option::is_none", rename = "folderId")]
    pub folder_id: Option<String>,
}
