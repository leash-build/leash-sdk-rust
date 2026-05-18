//! Typed integration namespaces — Gmail, Calendar, Drive, Linear, plus the
//! generic [`Provider`] escape hatch for providers without typed wrappers yet
//! (Slack, GitHub, HubSpot, …).
//!
//! Mirrors `leash.integrations` in the TS / Python / Go SDKs. Every call
//! routes through `crate::transport::Transport` which enforces the platform
//! contract (X-API-Key + Cookie only, never `Authorization: Bearer`).

mod calendar;
mod drive;
mod gmail;
mod linear;
pub mod types;

pub use calendar::Calendar;
pub use drive::Drive;
pub use gmail::Gmail;
pub use linear::Linear;

use crate::errors::Result;
use crate::transport::Transport;

/// `leash.integrations()` — typed providers plus a generic escape hatch.
#[derive(Debug, Clone)]
pub struct Integrations {
    transport: Transport,
}

impl Integrations {
    pub(crate) fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// Gmail provider.
    pub fn gmail(&self) -> Gmail {
        Gmail::new(self.transport.clone())
    }

    /// Google Calendar provider (canonical name).
    pub fn calendar(&self) -> Calendar {
        Calendar::new(self.transport.clone())
    }

    /// Alias of [`Self::calendar`] matching the TS / Python long-form name.
    pub fn google_calendar(&self) -> Calendar {
        self.calendar()
    }

    /// Google Drive provider (canonical name).
    pub fn drive(&self) -> Drive {
        Drive::new(self.transport.clone())
    }

    /// Alias of [`Self::drive`] matching the TS / Python long-form name.
    pub fn google_drive(&self) -> Drive {
        self.drive()
    }

    /// Linear provider.
    pub fn linear(&self) -> Linear {
        Linear::new(self.transport.clone())
    }

    /// Generic escape hatch — call any provider action by name.
    ///
    /// Use this for providers without typed wrappers yet (Slack, GitHub,
    /// HubSpot, Jira, Gong, Slite, BigQuery, custom MCP servers, …).
    ///
    /// ```no_run
    /// # async fn example(client: &leash_sdk::Leash) -> leash_sdk::Result<()> {
    /// let res = client
    ///     .integrations()
    ///     .provider("slack")
    ///     .call("post_message", serde_json::json!({ "channel": "#general", "text": "hi" }))
    ///     .await?;
    /// # let _ = res;
    /// # Ok(()) }
    /// ```
    pub fn provider(&self, name: impl Into<String>) -> Provider {
        Provider {
            transport: self.transport.clone(),
            name: name.into(),
        }
    }
}

/// Generic provider invoker — no typed shape, just a pass-through of action +
/// body. Returned by [`Integrations::provider`].
#[derive(Debug, Clone)]
pub struct Provider {
    transport: Transport,
    name: String,
}

impl Provider {
    /// Provider id this caller is bound to.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// POST to `/api/integrations/{name}/{action}` with the given body and
    /// return the raw JSON `data` field (or the raw response when there's no
    /// envelope).
    pub async fn call(&self, action: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        self.transport.integrations_call(&self.name, action, &body).await
    }
}
