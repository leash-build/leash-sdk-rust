//! # leash-sdk
//!
//! Rust SDK for the [Leash](https://leash.build) platform — one unified async
//! client for authentication, runtime env vars, and integrations.
//!
//! ## Quick start
//!
//! ```no_run
//! use leash_sdk::{Leash, GmailListParams};
//!
//! # async fn example(req: http::Request<()>) -> leash_sdk::Result<()> {
//! // Construct from any framework request (axum, actix-web, plain http::Request,
//! // anything implementing `LeashRequest`).
//! let leash = Leash::new(&req)?;
//!
//! // Identity
//! let user = leash.auth().user().await?;
//!
//! // Runtime env vars (60s TTL cache, dedicated get_fresh for cache-bypass)
//! let openai_key = leash.env().get("OPENAI_API_KEY").await?;
//!
//! // Integrations
//! let msgs = leash.integrations().gmail().list_messages(GmailListParams {
//!     max_results: Some(5),
//!     ..Default::default()
//! }).await?;
//! # let _ = (user, openai_key, msgs);
//! # Ok(()) }
//! ```
//!
//! ## Constructors
//!
//! | Use case | Constructor |
//! |---|---|
//! | Request-bound (axum, actix, plain http::Request) | [`Leash::new`] |
//! | Server-to-server | [`Leash::from_api_key`] |
//! | CLI / agent with a user JWT | [`Leash::from_token`] |
//!
//! ## Auth precedence
//!
//! 1. `LEASH_API_KEY` env var (or [`Leash::with_api_key`])
//! 2. `Authorization: Bearer <jwt>` header — used for identity and, when no
//!    API key is configured, as a fallback for env-fetch. **Never** forwarded
//!    on integration POSTs.
//! 3. `leash-auth` cookie — forwarded to the platform for integration calls.
//!
//! ## Errors
//!
//! [`LeashError`] is a structured enum with convenience predicates:
//!
//! ```no_run
//! # fn handle(err: leash_sdk::LeashError) {
//! if err.is_plan_block() { /* show upgrade UI */ }
//! if err.is_connection_required() { /* show "connect Gmail" CTA */ }
//! if err.is_unauthorized() { /* re-auth */ }
//! # }
//! ```

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod auth;
mod client;
mod env;
mod errors;
pub mod integrations;
mod request;
mod transport;

// ---------------------------------------------------------------------------
// Public surface — kept flat so consumers can `use leash_sdk::{Leash, Result, ...}`
// ---------------------------------------------------------------------------

pub use auth::{decode_user, get_leash_user, is_authenticated, Auth, LeashUser, LEASH_AUTH_COOKIE};
pub use client::{Leash, DEFAULT_PLATFORM_URL};
pub use env::{Env, ENV_CACHE_TTL};
pub use errors::{LeashError, Result};
pub use integrations::{Calendar, Drive, Gmail, Integrations, Linear, Provider};
pub use request::LeashRequest;

// Re-export integration types at the crate root so callers can write
// `use leash_sdk::GmailListParams;` instead of going through the submodule.
pub use integrations::types::{
    CalendarAttendee, CalendarCreateEventParams, CalendarEvent, CalendarEventList, CalendarList,
    CalendarListEntry, CalendarListEventsParams, ConnectionStatus, DriveFile, DriveFileList,
    DriveListFilesParams, DriveUploadFileParams, EventDateTime, GmailLabel, GmailLabelList,
    GmailListParams, GmailMessage, GmailMessageFormat, GmailMessageList, GmailSendMessageParams,
    Headers, LinearComment, LinearCreateIssueInput, LinearIssue, LinearListIssuesFilter,
    LinearListIssuesResult, LinearListProjectsFilter, LinearProject, LinearStateRef, LinearTeam,
    LinearTeamRef, LinearUpdateIssuePatch, LinearUserRef,
};

/// Convenience marker for callers who want to write
/// `leash.env().get(key, EnvFresh)` style code — see [`Env::get_fresh`] for
/// the actual cache-bypass entry point. Kept as a unit struct for parity with
/// the brief and to give docs a single anchor.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvFresh;
