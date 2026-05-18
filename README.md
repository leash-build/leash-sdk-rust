# leash-sdk (Rust)

Rust SDK for the [Leash](https://leash.build) platform — one unified async client for authentication, runtime env vars, and integrations.

Framework-agnostic. Works with axum, actix-web, plain `http::Request`, and anything else you can extract cookies + headers from.

## Installation

```toml
[dependencies]
leash-sdk = "0.4"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

# Optional framework integrations — pick what you use.
# leash-sdk = { version = "0.4", features = ["axum"] }
# leash-sdk = { version = "0.4", features = ["actix-web"] }
```

## Quick Start

```rust
use leash_sdk::{Leash, GmailListParams};

#[tokio::main]
async fn main() -> leash_sdk::Result<()> {
    // Server-to-server (CLI / agent / cron).
    let leash = Leash::from_api_key(std::env::var("LEASH_API_KEY").unwrap())?;

    // Identity
    let _ = leash.auth().user().await?;

    // Env vars (60s TTL cache, dedicated get_fresh for cache-bypass)
    let openai_key = leash.env().get("OPENAI_API_KEY").await?;

    // Integrations — typed verbs through the platform proxy.
    let msgs = leash.integrations().gmail().list_messages(GmailListParams {
        max_results: Some(5),
        ..Default::default()
    }).await?;

    println!("openai_key set? {}", openai_key.is_some());
    println!("messages: {}", msgs.messages.len());
    Ok(())
}
```

## Request-bound construction

The canonical entry point: `Leash::new(req)` reads `LEASH_API_KEY` from env, plus the inbound request's `leash-auth` cookie and `Authorization: Bearer` header.

### axum

```rust,no_run
use axum::{routing::get, Router, extract::Request};
use leash_sdk::Leash;

async fn me(req: Request) -> Result<String, String> {
    let leash = Leash::new(&req).map_err(|e| e.to_string())?;
    let user = leash.auth().user().await.map_err(|e| e.to_string())?;
    Ok(format!("hi {}", user.map(|u| u.name).unwrap_or_default()))
}

let _app = Router::<()>::new().route("/me", get(me));
```

### actix-web

```rust,no_run
use actix_web::{get, HttpRequest, Responder};
use leash_sdk::Leash;

#[get("/me")]
async fn me(req: HttpRequest) -> impl Responder {
    let leash = Leash::new(&req).unwrap();
    let user = leash.auth().user().await.unwrap();
    format!("hi {}", user.map(|u| u.name).unwrap_or_default())
}
```

### Plain `http::Request`

```rust
use leash_sdk::Leash;

let req = http::Request::builder()
    .header("cookie", "leash-auth=…")
    .body(()).unwrap();
let leash = Leash::new(&req).unwrap();
```

## Auth precedence

1. `LEASH_API_KEY` env var (or `Leash::with_api_key(...)`).
2. `Authorization: Bearer <jwt>` on the inbound request — used for identity and, **only when no API key is configured**, as a fallback bearer on env-fetch endpoints. **Never** forwarded on integration POSTs (the platform's `verifyToken()` rejects user JWTs before the API-key check runs).
3. `leash-auth` cookie — forwarded to the platform on integration calls.

## Integration verbs

| Provider | Verbs |
|---|---|
| `gmail()` | `list_messages`, `get_message`, `send_message`, `search_messages`, `list_labels`, `get_profile` |
| `calendar()` (alias `google_calendar()`) | `list_calendars`, `list_events`, `create_event`, `get_event` |
| `drive()` (alias `google_drive()`) | `list_files`, `get_file`, `download_file`, `create_folder`, `upload_file`, `delete_file`, `search_files` |
| `linear()` | `list_issues`, `get_issue`, `create_issue`, `update_issue`, `add_comment`, `list_teams`, `list_projects` |
| `provider(name)` | `call(action, body)` — generic escape hatch for Slack, GitHub, HubSpot, … |

```rust,no_run
use leash_sdk::{Leash, LinearListIssuesFilter};
# async fn ex() -> leash_sdk::Result<()> {
let leash = Leash::from_api_key("lsk_…")?;

let issues = leash.integrations().linear().list_issues(LinearListIssuesFilter {
    state_type: Some("started".into()),
    ..Default::default()
}).await?;

let res = leash.integrations().provider("slack").call(
    "post_message",
    serde_json::json!({ "channel": "#general", "text": "hi" }),
).await?;
# let _ = (issues, res); Ok(()) }
```

## Errors

`LeashError` is a structured enum. Predicates cover the common branches:

```rust
# fn handle(err: leash_sdk::LeashError) {
if err.is_plan_block()          { /* show upgrade UI */ }
if err.is_connection_required() { /* show "connect Gmail" CTA */ }
if err.is_unauthorized()        { /* re-auth */ }
if err.is_key_not_declared()    { /* show developer-facing hint */ }
# }
```

`env().get(key)` returns `Result<Option<String>>` — `Ok(None)` for KEY_NOT_DECLARED, `Err(_)` for actual errors so you can branch with `if value.is_none()`.

## TLS

`reqwest` is configured with `rustls-tls` only — no OpenSSL transitively.

## What's NOT in 0.4 yet

- **Dev-auth cookie-exchange handler** (`Leash::createDevAuthHandler` in TS) — the local-dev `/api/leash/dev-auth` flow is TS-only for 0.4. Rust callers using local dev should rely on the cookie pre-set by `leash up` instead.
- **Browser-mode client** — there's no WASM/browser build. Construct from a server route.
- **MCP exec helpers** (`run_mcp`, custom MCP config) — out of scope for the unified 0.4 client. Use the generic `provider(name).call(...)` escape hatch if you need to reach an MCP-backed integration directly.
- **Connection status / connect URL helpers** (`is_connected`, `get_connect_url`, `get_connections`) — surface to be re-added in a follow-up once the platform's `/api/integrations/connections` shape stabilises.

If you depended on any of these in 0.3.x, pin to `leash-sdk = "0.3"` and migrate when the replacement lands.

## Compatibility

| SDK | Wire-compatible with platform |
|---|---|
| 0.4.x | leash.build (current) |
| 0.3.x | leash.build (legacy `LeashIntegrations` surface) |

## License

Apache-2.0.
