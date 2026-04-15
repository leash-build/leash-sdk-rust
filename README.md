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

## Notes

- pass a valid Leash platform JWT as the auth token
- use `with_api_key(...)` for app-scoped access when needed
- provider OAuth remains a platform concern, not an SDK concern

## License

Apache-2.0
