//! Google Calendar provider.

use serde_json::json;

use crate::errors::Result;
use crate::transport::Transport;

use super::types::{
    CalendarCreateEventParams, CalendarEvent, CalendarEventList, CalendarList,
    CalendarListEventsParams,
};

const PROVIDER: &str = "google_calendar";

/// Typed Google Calendar provider client. Obtain via
/// `leash.integrations().calendar()` or `.google_calendar()`.
#[derive(Debug, Clone)]
pub struct Calendar {
    transport: Transport,
}

impl Calendar {
    pub(crate) fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// List the user's accessible calendars.
    pub async fn list_calendars(&self) -> Result<CalendarList> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list-calendars", &json!({}))
            .await?;
        decode(raw)
    }

    /// List events on a calendar. Pass `Default::default()` to use platform defaults.
    pub async fn list_events(
        &self,
        params: CalendarListEventsParams,
    ) -> Result<CalendarEventList> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "list-events", &params)
            .await?;
        decode(raw)
    }

    /// Create a new calendar event.
    pub async fn create_event(&self, params: CalendarCreateEventParams) -> Result<CalendarEvent> {
        let raw = self
            .transport
            .integrations_call(PROVIDER, "create-event", &params)
            .await?;
        decode(raw)
    }

    /// Retrieve a single event. Pass `None` for `calendar_id` to use the primary calendar.
    pub async fn get_event(
        &self,
        event_id: &str,
        calendar_id: Option<&str>,
    ) -> Result<CalendarEvent> {
        let body = if let Some(cid) = calendar_id {
            json!({ "eventId": event_id, "calendarId": cid })
        } else {
            json!({ "eventId": event_id })
        };
        let raw = self
            .transport
            .integrations_call(PROVIDER, "get-event", &body)
            .await?;
        decode(raw)
    }
}

fn decode<T: serde::de::DeserializeOwned + Default>(raw: serde_json::Value) -> Result<T> {
    if raw.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(raw).map_err(|e| crate::errors::LeashError::MalformedResponse {
        message: format!("Failed to deserialise Calendar response: {e}"),
    })
}
