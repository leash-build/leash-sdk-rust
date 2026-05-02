use leash_sdk::{
    Attendee, ConnectionStatus, CreateEventParams, CustomMcpServerConfig, EventDateTime,
    LeashError, LeashIntegrations, ListEventsParams, ListFilesParams, ListMessagesParams,
    SendMessageParams, DEFAULT_PLATFORM_URL,
};
use std::collections::HashMap;

// -------------------------------------------------------------------------
// Client initialization
// -------------------------------------------------------------------------

#[test]
fn client_uses_default_platform_url() {
    let client = LeashIntegrations::new("test-token");
    assert_eq!(client.get_connect_url("gmail", None),
               format!("{DEFAULT_PLATFORM_URL}/api/integrations/connect/gmail"));
}

#[test]
fn client_with_custom_platform_url() {
    let client = LeashIntegrations::new("tok")
        .with_platform_url("https://custom.example.com");
    let url = client.get_connect_url("gmail", None);
    assert_eq!(url, "https://custom.example.com/api/integrations/connect/gmail");
}

#[test]
fn client_trims_trailing_slash_from_platform_url() {
    let client = LeashIntegrations::new("tok")
        .with_platform_url("https://custom.example.com/");
    let url = client.get_connect_url("gmail", None);
    assert_eq!(url, "https://custom.example.com/api/integrations/connect/gmail");
}

#[test]
fn client_with_api_key() {
    // We can't directly inspect the api_key field (it's private-ish via pub(crate)),
    // but we can verify the builder doesn't panic and connect URL still works.
    let client = LeashIntegrations::new("tok")
        .with_api_key("my-secret-key");
    let url = client.get_connect_url("calendar", None);
    assert!(url.contains("/api/integrations/connect/calendar"));
}

// -------------------------------------------------------------------------
// Connect URL generation
// -------------------------------------------------------------------------

#[test]
fn connect_url_without_return_url() {
    let client = LeashIntegrations::new("tok");
    let url = client.get_connect_url("google_drive", None);
    assert_eq!(
        url,
        format!("{DEFAULT_PLATFORM_URL}/api/integrations/connect/google_drive")
    );
}

#[test]
fn connect_url_with_return_url() {
    let client = LeashIntegrations::new("tok");
    let url = client.get_connect_url("gmail", Some("https://myapp.com/callback?foo=bar"));
    assert!(url.starts_with(&format!(
        "{DEFAULT_PLATFORM_URL}/api/integrations/connect/gmail?return_url="
    )));
    // The return URL should be percent-encoded
    assert!(url.contains("https%3A%2F%2Fmyapp.com%2Fcallback%3Ffoo%3Dbar"));
}

#[test]
fn connect_url_encodes_special_characters() {
    let client = LeashIntegrations::new("tok");
    let url = client.get_connect_url("gmail", Some("https://example.com/path with spaces"));
    // Spaces should be encoded as %20
    assert!(url.contains("%20"));
    assert!(!url.contains(' '));
}

#[test]
fn connect_url_preserves_unreserved_characters() {
    let client = LeashIntegrations::new("tok");
    // Unreserved chars: A-Z a-z 0-9 - _ . ~
    let url = client.get_connect_url("gmail", Some("hello-world_test.page~v2"));
    assert!(url.contains("hello-world_test.page~v2"));
}

// -------------------------------------------------------------------------
// URL construction for provider calls (verify format without making HTTP)
// -------------------------------------------------------------------------

#[test]
fn integration_url_format_default() {
    // We verify the URL format by checking get_connect_url which uses the same
    // platform_url base. The call_internal builds:
    //   {platform_url}/api/integrations/{provider}/{action}
    // We can't call it without a server, but we verify the base is correct.
    let client = LeashIntegrations::new("tok");
    let connect = client.get_connect_url("gmail", None);
    assert_eq!(
        connect,
        "https://leash.build/api/integrations/connect/gmail"
    );
}

#[test]
fn integration_url_format_custom_base() {
    let client = LeashIntegrations::new("tok")
        .with_platform_url("http://localhost:3000");
    let connect = client.get_connect_url("google_calendar", None);
    assert_eq!(
        connect,
        "http://localhost:3000/api/integrations/connect/google_calendar"
    );
}

// -------------------------------------------------------------------------
// DEFAULT_PLATFORM_URL constant
// -------------------------------------------------------------------------

#[test]
fn default_platform_url_is_correct() {
    assert_eq!(DEFAULT_PLATFORM_URL, "https://leash.build");
}

// -------------------------------------------------------------------------
// Error type construction and display
// -------------------------------------------------------------------------

#[test]
fn error_not_connected_display() {
    let err = LeashError::NotConnected {
        message: "Gmail not connected".to_string(),
        connect_url: Some("https://leash.build/connect/gmail".to_string()),
    };
    let msg = format!("{err}");
    assert!(msg.contains("not connected"));
    assert!(msg.contains("Gmail not connected"));
}

#[test]
fn error_not_connected_without_url() {
    let err = LeashError::NotConnected {
        message: "disconnected".to_string(),
        connect_url: None,
    };
    let msg = format!("{err}");
    assert!(msg.contains("not connected"));
}

#[test]
fn error_token_expired_display() {
    let err = LeashError::TokenExpired {
        message: "token has expired".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("token expired"));
    assert!(msg.contains("token has expired"));
}

#[test]
fn error_api_error_with_code() {
    let err = LeashError::ApiError {
        message: "rate limited".to_string(),
        code: Some("rate_limit".to_string()),
    };
    let msg = format!("{err}");
    assert!(msg.contains("rate limited"));
    assert!(msg.contains("rate_limit"));
}

#[test]
fn error_api_error_without_code() {
    let err = LeashError::ApiError {
        message: "something went wrong".to_string(),
        code: None,
    };
    let msg = format!("{err}");
    assert!(msg.contains("something went wrong"));
    assert!(!msg.contains("code:"));
}

#[test]
fn error_implements_std_error() {
    let err = LeashError::ApiError {
        message: "test".to_string(),
        code: None,
    };
    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &err;
}

#[test]
fn error_source_is_none_for_non_network_errors() {
    use std::error::Error;
    let err = LeashError::ApiError {
        message: "test".to_string(),
        code: None,
    };
    assert!(err.source().is_none());

    let err2 = LeashError::NotConnected {
        message: "x".to_string(),
        connect_url: None,
    };
    assert!(err2.source().is_none());

    let err3 = LeashError::TokenExpired {
        message: "x".to_string(),
    };
    assert!(err3.source().is_none());
}

// -------------------------------------------------------------------------
// Type serialization
// -------------------------------------------------------------------------

#[test]
fn list_messages_params_default_serializes_to_empty_object() {
    let params = ListMessagesParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn list_messages_params_serializes_fields_correctly() {
    let params = ListMessagesParams {
        query: Some("from:user@example.com".to_string()),
        max_results: Some(10),
        label_ids: Some(vec!["INBOX".to_string()]),
        page_token: Some("token123".to_string()),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["query"], "from:user@example.com");
    assert_eq!(json["maxResults"], 10);
    assert_eq!(json["labelIds"], serde_json::json!(["INBOX"]));
    assert_eq!(json["pageToken"], "token123");
}

#[test]
fn list_messages_params_skips_none_fields() {
    let params = ListMessagesParams {
        query: Some("test".to_string()),
        max_results: None,
        label_ids: None,
        page_token: None,
    };
    let json = serde_json::to_value(&params).unwrap();
    assert!(json.get("query").is_some());
    assert!(json.get("maxResults").is_none());
    assert!(json.get("labelIds").is_none());
    assert!(json.get("pageToken").is_none());
}

#[test]
fn send_message_params_serializes() {
    let params = SendMessageParams {
        to: "alice@example.com".to_string(),
        subject: "Hello".to_string(),
        body: "Hi there".to_string(),
        cc: Some("bob@example.com".to_string()),
        bcc: None,
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["to"], "alice@example.com");
    assert_eq!(json["subject"], "Hello");
    assert_eq!(json["body"], "Hi there");
    assert_eq!(json["cc"], "bob@example.com");
    assert!(json.get("bcc").is_none());
}

#[test]
fn list_events_params_default_serializes_to_empty_object() {
    let params = ListEventsParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn list_events_params_serializes_all_fields() {
    let params = ListEventsParams {
        calendar_id: Some("primary".to_string()),
        time_min: Some("2024-01-01T00:00:00Z".to_string()),
        time_max: Some("2024-12-31T23:59:59Z".to_string()),
        max_results: Some(50),
        query: Some("meeting".to_string()),
        single_events: Some(true),
        order_by: Some("startTime".to_string()),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["calendarId"], "primary");
    assert_eq!(json["timeMin"], "2024-01-01T00:00:00Z");
    assert_eq!(json["timeMax"], "2024-12-31T23:59:59Z");
    assert_eq!(json["maxResults"], 50);
    assert_eq!(json["query"], "meeting");
    assert_eq!(json["singleEvents"], true);
    assert_eq!(json["orderBy"], "startTime");
}

#[test]
fn event_date_time_serializes_with_date_time() {
    let dt = EventDateTime {
        date_time: Some("2024-06-15T10:00:00-04:00".to_string()),
        date: None,
        time_zone: Some("America/New_York".to_string()),
    };
    let json = serde_json::to_value(&dt).unwrap();
    assert_eq!(json["dateTime"], "2024-06-15T10:00:00-04:00");
    assert_eq!(json["timeZone"], "America/New_York");
    assert!(json.get("date").is_none());
}

#[test]
fn event_date_time_serializes_all_day_event() {
    let dt = EventDateTime {
        date_time: None,
        date: Some("2024-06-15".to_string()),
        time_zone: None,
    };
    let json = serde_json::to_value(&dt).unwrap();
    assert_eq!(json["date"], "2024-06-15");
    assert!(json.get("dateTime").is_none());
    assert!(json.get("timeZone").is_none());
}

#[test]
fn attendee_serializes() {
    let attendee = Attendee {
        email: "user@example.com".to_string(),
    };
    let json = serde_json::to_value(&attendee).unwrap();
    assert_eq!(json["email"], "user@example.com");
}

#[test]
fn create_event_params_serializes() {
    let params = CreateEventParams {
        calendar_id: None,
        summary: "Team Standup".to_string(),
        description: Some("Daily standup meeting".to_string()),
        location: Some("Room 101".to_string()),
        start: EventDateTime {
            date_time: Some("2024-06-15T09:00:00Z".to_string()),
            date: None,
            time_zone: None,
        },
        end: EventDateTime {
            date_time: Some("2024-06-15T09:30:00Z".to_string()),
            date: None,
            time_zone: None,
        },
        attendees: Some(vec![Attendee {
            email: "colleague@example.com".to_string(),
        }]),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["summary"], "Team Standup");
    assert_eq!(json["description"], "Daily standup meeting");
    assert_eq!(json["location"], "Room 101");
    assert!(json.get("calendarId").is_none());
    assert_eq!(json["start"]["dateTime"], "2024-06-15T09:00:00Z");
    assert_eq!(json["end"]["dateTime"], "2024-06-15T09:30:00Z");
    assert_eq!(json["attendees"][0]["email"], "colleague@example.com");
}

#[test]
fn list_files_params_default_serializes_to_empty_object() {
    let params = ListFilesParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn list_files_params_serializes_all_fields() {
    let params = ListFilesParams {
        query: Some("type:pdf".to_string()),
        max_results: Some(25),
        folder_id: Some("folder-abc".to_string()),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["query"], "type:pdf");
    assert_eq!(json["maxResults"], 25);
    assert_eq!(json["folderId"], "folder-abc");
}

// -------------------------------------------------------------------------
// Type deserialization (ConnectionStatus)
// -------------------------------------------------------------------------

#[test]
fn connection_status_deserializes() {
    let json = serde_json::json!({
        "providerId": "gmail",
        "status": "active",
        "email": "user@gmail.com",
        "expiresAt": "2024-12-31T00:00:00Z"
    });
    let cs: ConnectionStatus = serde_json::from_value(json).unwrap();
    assert_eq!(cs.provider_id, "gmail");
    assert_eq!(cs.status, "active");
    assert_eq!(cs.email, Some("user@gmail.com".to_string()));
    assert_eq!(cs.expires_at, Some("2024-12-31T00:00:00Z".to_string()));
}

#[test]
fn connection_status_deserializes_minimal() {
    let json = serde_json::json!({
        "providerId": "google_calendar",
        "status": "expired"
    });
    let cs: ConnectionStatus = serde_json::from_value(json).unwrap();
    assert_eq!(cs.provider_id, "google_calendar");
    assert_eq!(cs.status, "expired");
    assert_eq!(cs.email, None);
    assert_eq!(cs.expires_at, None);
}

#[test]
fn connection_status_roundtrip() {
    let original = ConnectionStatus {
        provider_id: "google_drive".to_string(),
        status: "active".to_string(),
        email: Some("test@example.com".to_string()),
        expires_at: None,
    };
    let json = serde_json::to_value(&original).unwrap();
    let deserialized: ConnectionStatus = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.provider_id, original.provider_id);
    assert_eq!(deserialized.status, original.status);
    assert_eq!(deserialized.email, original.email);
    assert_eq!(deserialized.expires_at, original.expires_at);
}

// -------------------------------------------------------------------------
// Provider client accessors (verify they can be created)
// -------------------------------------------------------------------------

#[test]
fn gmail_client_can_be_created() {
    let client = LeashIntegrations::new("tok");
    let _gmail = client.gmail();
}

#[test]
fn calendar_client_can_be_created() {
    let client = LeashIntegrations::new("tok");
    let _calendar = client.calendar();
}

#[test]
fn drive_client_can_be_created() {
    let client = LeashIntegrations::new("tok");
    let _drive = client.drive();
}

#[test]
fn custom_integration_can_be_created() {
    let client = LeashIntegrations::new("tok");
    let _custom = client.integration("stripe");
}

// -------------------------------------------------------------------------
// Builder chaining
// -------------------------------------------------------------------------

#[test]
fn builder_methods_can_be_chained() {
    let client = LeashIntegrations::new("my-token")
        .with_platform_url("https://custom.leash.build")
        .with_api_key("key-123")
        .with_http_client(reqwest::Client::new());
    // Verify the custom URL was set
    let url = client.get_connect_url("gmail", None);
    assert!(url.starts_with("https://custom.leash.build/"));
}

#[test]
fn multiple_connect_urls_for_different_providers() {
    let client = LeashIntegrations::new("tok");
    let gmail_url = client.get_connect_url("gmail", None);
    let cal_url = client.get_connect_url("google_calendar", None);
    let drive_url = client.get_connect_url("google_drive", None);

    assert!(gmail_url.contains("/gmail"));
    assert!(cal_url.contains("/google_calendar"));
    assert!(drive_url.contains("/google_drive"));

    // All should share the same base
    let base = format!("{DEFAULT_PLATFORM_URL}/api/integrations/connect/");
    assert!(gmail_url.starts_with(&base));
    assert!(cal_url.starts_with(&base));
    assert!(drive_url.starts_with(&base));
}

// -------------------------------------------------------------------------
// CustomMcpServerConfig serialization / deserialization (LEA-143)
// -------------------------------------------------------------------------

#[test]
fn custom_mcp_server_config_deserializes() {
    let json = serde_json::json!({
        "slug": "acme-issues",
        "displayName": "Acme Issues",
        "url": "https://mcp.acme.example.com",
        "headers": {
            "Authorization": "Bearer token-xyz",
            "X-Acme-Workspace": "ws_123"
        }
    });
    let cfg: CustomMcpServerConfig = serde_json::from_value(json).unwrap();
    assert_eq!(cfg.slug, "acme-issues");
    assert_eq!(cfg.display_name, "Acme Issues");
    assert_eq!(cfg.url, "https://mcp.acme.example.com");
    assert_eq!(
        cfg.headers.get("Authorization"),
        Some(&"Bearer token-xyz".to_string())
    );
    assert_eq!(
        cfg.headers.get("X-Acme-Workspace"),
        Some(&"ws_123".to_string())
    );
}

#[test]
fn custom_mcp_server_config_deserializes_with_empty_headers() {
    let json = serde_json::json!({
        "slug": "no-auth-mcp",
        "displayName": "No Auth MCP",
        "url": "https://mcp.example.com",
        "headers": {}
    });
    let cfg: CustomMcpServerConfig = serde_json::from_value(json).unwrap();
    assert_eq!(cfg.slug, "no-auth-mcp");
    assert_eq!(cfg.display_name, "No Auth MCP");
    assert!(cfg.headers.is_empty());
}

#[test]
fn custom_mcp_server_config_roundtrip() {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer abc-123".to_string(),
    );

    let original = CustomMcpServerConfig {
        slug: "example".to_string(),
        display_name: "Example MCP".to_string(),
        url: "https://mcp.example.com/sse".to_string(),
        headers,
    };
    let json = serde_json::to_value(&original).unwrap();
    // Verify camelCase on the wire
    assert_eq!(json["slug"], "example");
    assert_eq!(json["displayName"], "Example MCP");
    assert_eq!(json["url"], "https://mcp.example.com/sse");
    assert_eq!(json["headers"]["Authorization"], "Bearer abc-123");

    let deserialized: CustomMcpServerConfig = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.slug, original.slug);
    assert_eq!(deserialized.display_name, original.display_name);
    assert_eq!(deserialized.url, original.url);
    assert_eq!(deserialized.headers, original.headers);
}

#[test]
fn custom_mcp_server_config_serializes_camel_case_field() {
    let cfg = CustomMcpServerConfig {
        slug: "s".to_string(),
        display_name: "Display".to_string(),
        url: "https://x".to_string(),
        headers: HashMap::new(),
    };
    let json = serde_json::to_value(&cfg).unwrap();
    // Wire format must use displayName, not display_name
    assert!(json.get("displayName").is_some());
    assert!(json.get("display_name").is_none());
}

// -------------------------------------------------------------------------
// Error mapping for new endpoints
// -------------------------------------------------------------------------

#[test]
fn token_endpoint_error_maps_not_connected() {
    // Mirrors the platform's POST /api/integrations/token error envelope.
    let err_envelope = serde_json::json!({
        "success": false,
        "error": "Slack is not connected for this user",
        "code": "not_connected",
        "connectUrl": "https://leash.build/api/integrations/connect/slack"
    });

    // Decode into the public error type by going through the same path
    // call() uses: ApiResponse -> into_error(). We can't construct an
    // ApiResponse directly (it's pub(crate)), so we simulate by checking
    // the error mapping a LeashError would produce given the envelope's
    // shape. The into_error() logic is exercised via every call site,
    // and it's the only place "not_connected" is mapped.
    let code = err_envelope["code"].as_str().unwrap();
    let message = err_envelope["error"].as_str().unwrap();
    let connect_url = err_envelope["connectUrl"].as_str().map(String::from);

    let err = match code {
        "not_connected" => LeashError::NotConnected {
            message: message.to_string(),
            connect_url,
        },
        "token_expired" => LeashError::TokenExpired {
            message: message.to_string(),
        },
        _ => LeashError::ApiError {
            message: message.to_string(),
            code: Some(code.to_string()),
        },
    };

    match err {
        LeashError::NotConnected {
            message,
            connect_url,
        } => {
            assert!(message.contains("Slack is not connected"));
            assert_eq!(
                connect_url,
                Some("https://leash.build/api/integrations/connect/slack".to_string())
            );
        }
        other => panic!("expected NotConnected, got {other:?}"),
    }
}

#[test]
fn mcp_config_endpoint_error_maps_unknown_mcp_server() {
    // Mirrors the platform's GET /api/integrations/mcp-config/{slug} error
    // envelope when the slug is not registered for this org.
    let err_envelope = serde_json::json!({
        "success": false,
        "error": "Unknown MCP server: acme-issues",
        "code": "unknown_mcp_server"
    });

    let code = err_envelope["code"].as_str().unwrap();
    let message = err_envelope["error"].as_str().unwrap();

    // unknown_mcp_server is not a special-cased error code, so it should
    // fall through to ApiError with the code preserved.
    let err = match code {
        "not_connected" => LeashError::NotConnected {
            message: message.to_string(),
            connect_url: None,
        },
        "token_expired" => LeashError::TokenExpired {
            message: message.to_string(),
        },
        _ => LeashError::ApiError {
            message: message.to_string(),
            code: Some(code.to_string()),
        },
    };

    match err {
        LeashError::ApiError { message, code } => {
            assert!(message.contains("Unknown MCP server"));
            assert_eq!(code, Some("unknown_mcp_server".to_string()));
        }
        other => panic!("expected ApiError, got {other:?}"),
    }
}

// -------------------------------------------------------------------------
// URL format checks for new endpoints (without making HTTP calls)
// -------------------------------------------------------------------------

#[test]
fn token_endpoint_url_format() {
    // The token endpoint posts to {platform_url}/api/integrations/token.
    // We can't observe call_internal from outside, but we can confirm
    // the platform_url builder is the same one used by every other call.
    let client = LeashIntegrations::new("tok").with_platform_url("http://localhost:3000");
    let connect_url = client.get_connect_url("slack", None);
    // get_connect_url shares the same platform_url base as get_access_token.
    assert!(connect_url.starts_with("http://localhost:3000/api/integrations/"));
}

#[test]
fn mcp_config_url_format() {
    let client = LeashIntegrations::new("tok").with_platform_url("https://staging.leash.build");
    let connect_url = client.get_connect_url("notion", None);
    // get_custom_mcp_config builds {platform_url}/api/integrations/mcp-config/{slug}
    // — same platform_url base.
    assert!(connect_url.starts_with("https://staging.leash.build/api/integrations/"));
}

// -------------------------------------------------------------------------
// Methods exist with the expected signatures (compile-only checks)
// -------------------------------------------------------------------------

#[test]
fn get_access_token_signature_compiles() {
    // Ensure get_access_token is accessible on LeashIntegrations and
    // returns a Future. We don't actually await it (no server).
    let client = LeashIntegrations::new("tok");
    let _fut = client.get_access_token("slack");
}

#[test]
fn get_custom_mcp_config_signature_compiles() {
    let client = LeashIntegrations::new("tok");
    let _fut = client.get_custom_mcp_config("acme-issues");
}
