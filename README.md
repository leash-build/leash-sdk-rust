# leash-sdk (Rust)

Rust SDK for the [Leash](https://leash.build) platform integrations API.

## Installation

```toml
[dependencies]
leash-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick start

```rust
use leash_sdk::{LeashIntegrations, ListMessagesParams, SendMessageParams};

#[tokio::main]
async fn main() -> Result<(), leash_sdk::LeashError> {
    let client = LeashIntegrations::new("your-jwt-token")
        .with_platform_url("https://leash.build")
        .with_api_key("optional-api-key");

    // Gmail
    let messages = client.gmail().list_messages(None).await?;
    let labels = client.gmail().list_labels().await?;

    // Calendar
    let calendars = client.calendar().list_calendars().await?;
    let events = client.calendar().list_events(None).await?;

    // Drive
    let files = client.drive().list_files(None).await?;

    // Connections
    let connected = client.is_connected("gmail").await;
    let connect_url = client.get_connect_url("gmail", Some("https://myapp.com/callback"));

    Ok(())
}
```

## API

### `LeashIntegrations`

| Method | Description |
|--------|-------------|
| `new(auth_token)` | Create client with default platform URL |
| `with_platform_url(url)` | Set custom platform URL |
| `with_api_key(key)` | Set API key for `X-API-Key` header |
| `gmail()` | Get Gmail client |
| `calendar()` | Get Calendar client |
| `drive()` | Get Drive client |
| `call(provider, action, body)` | Generic integration call |
| `is_connected(provider_id)` | Check if a provider is connected |
| `get_connections()` | Get all connection statuses |
| `get_connect_url(provider_id, return_url)` | Get OAuth connect URL |

### Gmail

- `list_messages(params)` - List messages
- `get_message(message_id, format)` - Get a message
- `send_message(params)` - Send a message
- `search_messages(query, max_results)` - Search messages
- `list_labels()` - List labels

### Calendar

- `list_calendars()` - List calendars
- `list_events(params)` - List events
- `create_event(params)` - Create an event
- `get_event(event_id, calendar_id)` - Get an event

### Drive

- `list_files(params)` - List files
- `get_file(file_id)` - Get file metadata
- `search_files(query, max_results)` - Search files

## License

MIT
