use crate::client::LeashIntegrations;
use crate::types::{CreateEventParams, LeashError, ListEventsParams};

const PROVIDER: &str = "google_calendar";

/// Client for the Google Calendar integration.
///
/// Obtained via [`LeashIntegrations::calendar()`].
pub struct CalendarClient<'a> {
    pub(crate) client: &'a LeashIntegrations,
}

impl<'a> CalendarClient<'a> {
    /// List all calendars accessible to the user.
    pub async fn list_calendars(&self) -> Result<serde_json::Value, LeashError> {
        self.client.call_internal(PROVIDER, "list-calendars", None).await
    }

    /// List events from a calendar.
    ///
    /// Pass `None` to use server defaults.
    pub async fn list_events(
        &self,
        params: Option<ListEventsParams>,
    ) -> Result<serde_json::Value, LeashError> {
        let body = params.map(|p| serde_json::to_value(p).unwrap());
        self.client.call_internal(PROVIDER, "list-events", body).await
    }

    /// Create a new calendar event.
    pub async fn create_event(
        &self,
        params: CreateEventParams,
    ) -> Result<serde_json::Value, LeashError> {
        let body = serde_json::to_value(params).unwrap();
        self.client.call_internal(PROVIDER, "create-event", Some(body)).await
    }

    /// Get a single event by ID.
    ///
    /// `calendar_id` is optional; if `None`, the server uses `"primary"`.
    pub async fn get_event(
        &self,
        event_id: &str,
        calendar_id: Option<&str>,
    ) -> Result<serde_json::Value, LeashError> {
        let mut body = serde_json::json!({ "eventId": event_id });
        if let Some(cal_id) = calendar_id {
            body["calendarId"] = serde_json::Value::String(cal_id.to_string());
        }
        self.client.call_internal(PROVIDER, "get-event", Some(body)).await
    }
}
