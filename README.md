# leash-sdk (Rust)

Rust SDK for Leash-hosted integrations.

Use it to call provider actions through the Leash platform proxy instead of handling provider OAuth and token storage yourself.

## Installation

```toml
[dependencies]
leash-sdk = "0.2"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use leash_sdk::LeashIntegrations;

#[tokio::main]
async fn main() -> Result<(), leash_sdk::LeashError> {
    let client = LeashIntegrations::new("your-platform-jwt")
        .with_platform_url("https://leash.build")
        .with_api_key("optional-app-api-key");

    let messages = client.gmail().list_messages(None).await?;
    let connected = client.is_connected("gmail").await;
    let connect_url = client.get_connect_url("gmail", Some("https://myapp.example.com/settings"));

    println!("connected: {}", connected);
    println!("messages: {}", messages);
    println!("connect url: {}", connect_url);
    Ok(())
}
```

## Default Platform URL

- `https://leash.build`

## Features

- Gmail
- Google Calendar
- Google Drive
- connection status lookup
- connect URL generation
- generic provider calls
- custom integration calls
- app env fetch and caching
- MCP execution through the platform

## Server Auth

The SDK includes framework-agnostic helpers for authenticating users on the
server side by reading the `leash-auth` cookie set by the Leash platform.

```rust
use leash_sdk::{get_leash_user, is_authenticated};

// In any handler that has access to the raw Cookie header:
let user = leash_sdk::get_leash_user(cookie_header)?;
println!("Hello, {}", user.name);

// Or just check authentication:
if leash_sdk::is_authenticated(cookie_header) {
    // proceed
}
```

If your framework has already parsed cookies, use the token directly:

```rust
let user = leash_sdk::get_leash_user_from_cookie(token)?;
```

## MCP Calls

Execute MCP-backed tools through the platform:

```rust
let result = client.run_mcp("@some/mcp-package", "tool-name", serde_json::json!({"key": "value"})).await?;
```

## Notes

- pass a valid Leash platform JWT as the auth token
- use `with_api_key(...)` for app-scoped access when needed
- provider OAuth remains a platform concern, not an SDK concern

## License

Apache-2.0
