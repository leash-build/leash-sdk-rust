//! # leash-sdk
//!
//! Rust SDK for the [Leash](https://leash.build) platform integrations API.
//!
//! The SDK communicates with the Leash platform proxy which handles OAuth tokens
//! and routes requests to Google Gmail, Calendar, and Drive APIs.
//!
//! ## Quick start
//!
//! ```no_run
//! use leash_sdk::LeashIntegrations;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), leash_sdk::LeashError> {
//!     let client = LeashIntegrations::new("your-jwt-token");
//!
//!     // List Gmail messages
//!     let messages = client.gmail().list_messages(None).await?;
//!     println!("{messages}");
//!
//!     // Check if Gmail is connected
//!     let connected = client.is_connected("gmail").await;
//!     println!("Gmail connected: {connected}");
//!
//!     Ok(())
//! }
//! ```

pub mod auth;
pub mod calendar;
pub mod client;
pub mod custom;
pub mod drive;
pub mod gmail;
pub mod types;

// Re-exports for convenience.
pub use auth::{get_leash_user, get_leash_user_from_cookie, LeashUser};
pub use client::LeashIntegrations;
pub use custom::CustomIntegration;
pub use types::{
    Attendee, ConnectionStatus, CreateEventParams, EventDateTime, LeashError, ListEventsParams,
    ListFilesParams, ListMessagesParams, SendMessageParams, DEFAULT_PLATFORM_URL,
};
