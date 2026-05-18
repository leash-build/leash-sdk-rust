//! End-to-end tests for the 0.4 unified client.
//!
//! Each test spins up a [`wiremock`] server and points the client at it via
//! [`Leash::with_platform_url`]. Tests cover constructors, auth precedence
//! (including the critical "Bearer never on integration POSTs" assertion),
//! every typed verb, and the structured error mapping.

use leash_sdk::{
    CalendarCreateEventParams, CalendarListEventsParams, DriveListFilesParams,
    DriveUploadFileParams, EventDateTime, GmailListParams, GmailMessageFormat,
    GmailSendMessageParams, Leash, LeashError, LinearCreateIssueInput, LinearListIssuesFilter,
    LinearListProjectsFilter, LinearUpdateIssuePatch,
};
use serde_json::{json, Value};
use wiremock::matchers::{body_json, body_partial_json, header, header_exists, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a Leash client wired to the given mock server with both API key and
/// cookie supplied — the common case for happy-path tests.
fn client_with_key_and_cookie(server: &MockServer, key: &str, cookie: &str) -> Leash {
    let req = http::Request::builder()
        .header("cookie", format!("leash-auth={cookie}"))
        .body(())
        .unwrap();
    // Drop the env var so we test the explicit key path.
    std::env::remove_var("LEASH_API_KEY");
    Leash::new(&req)
        .unwrap()
        .with_platform_url(server.uri())
        .with_api_key(key)
}

fn client_with_key(server: &MockServer, key: &str) -> Leash {
    std::env::remove_var("LEASH_API_KEY");
    Leash::from_api_key(key).unwrap().with_platform_url(server.uri())
}

fn client_with_bearer_only(server: &MockServer, bearer: &str) -> Leash {
    std::env::remove_var("LEASH_API_KEY");
    let req = http::Request::builder()
        .header("authorization", format!("Bearer {bearer}"))
        .body(())
        .unwrap();
    Leash::new(&req).unwrap().with_platform_url(server.uri())
}

fn ok_envelope(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": data }))
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn from_api_key_constructs() {
    std::env::remove_var("LEASH_API_KEY");
    let leash = Leash::from_api_key("lsk_test").unwrap();
    assert!(leash.has_api_key());
    assert!(!leash.has_cookie());
    assert!(!leash.has_bearer());
}

#[tokio::test]
async fn from_api_key_rejects_empty() {
    assert!(Leash::from_api_key("").is_err());
}

#[tokio::test]
async fn from_token_constructs() {
    let leash = Leash::from_token("jwt-abc").unwrap();
    assert!(leash.has_cookie());
    assert!(leash.has_bearer());
}

#[tokio::test]
async fn new_extracts_cookie_and_bearer() {
    std::env::remove_var("LEASH_API_KEY");
    let req = http::Request::builder()
        .header("cookie", "leash-auth=cookie-jwt")
        .header("authorization", "Bearer header-jwt")
        .body(())
        .unwrap();
    let leash = Leash::new(&req).unwrap();
    assert!(leash.has_cookie());
    assert!(leash.has_bearer());
    assert!(!leash.has_api_key());
}

#[tokio::test]
async fn platform_url_defaults_to_leash_build() {
    std::env::remove_var("LEASH_API_KEY");
    std::env::remove_var("LEASH_PLATFORM_URL");
    let leash = Leash::from_api_key("k").unwrap();
    assert_eq!(leash.platform_url(), "https://leash.build");
}

#[tokio::test]
async fn platform_url_respects_env_var() {
    std::env::remove_var("LEASH_API_KEY");
    std::env::set_var("LEASH_PLATFORM_URL", "https://staging.leash.build");
    let leash = Leash::from_api_key("k").unwrap();
    assert_eq!(leash.platform_url(), "https://staging.leash.build");
    std::env::remove_var("LEASH_PLATFORM_URL");
}

// ---------------------------------------------------------------------------
// Auth precedence — the critical negative assertion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_call_sends_x_api_key_and_cookie_only() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "lsk_test", "leash-jwt");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .and(header("X-API-Key", "lsk_test"))
        .and(header("Cookie", "leash-auth=leash-jwt"))
        .respond_with(ok_envelope(json!({ "messages": [], "resultSizeEstimate": 0 })))
        .mount(&server)
        .await;

    let res = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap();
    assert_eq!(res.messages.len(), 0);
}

#[tokio::test]
async fn integration_call_never_forwards_bearer_token() {
    // This is the platform's hard contract — sending Authorization: Bearer on
    // /api/integrations/* triggers verifyToken() before the X-API-Key check
    // runs and produces a phantom 401. Assert we never send it.
    let server = MockServer::start().await;
    std::env::remove_var("LEASH_API_KEY");

    let req = http::Request::builder()
        .header("cookie", "leash-auth=cookie-jwt")
        .header("authorization", "Bearer should_not_forward")
        .body(())
        .unwrap();
    let leash = Leash::new(&req)
        .unwrap()
        .with_platform_url(server.uri())
        .with_api_key("lsk_test");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .respond_with(ok_envelope(json!({ "messages": [] })))
        .mount(&server)
        .await;

    leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap();

    // Inspect the captured request — Authorization must be absent.
    let received = server.received_requests().await.unwrap();
    let first = &received[0];
    assert!(
        first.headers.get("authorization").is_none()
            && first.headers.get("Authorization").is_none(),
        "Authorization header must NOT be forwarded on integration POSTs"
    );
    // X-API-Key and Cookie should both be present.
    assert!(first.headers.contains_key("X-API-Key") || first.headers.contains_key("x-api-key"));
    assert!(first.headers.contains_key("Cookie") || first.headers.contains_key("cookie"));
}

#[tokio::test]
async fn env_fetch_uses_api_key_as_bearer() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "lsk_secret");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/OPENAI_API_KEY"))
        .and(header("Authorization", "Bearer lsk_secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "sk-abc" })))
        .mount(&server)
        .await;

    let val = leash.env().get("OPENAI_API_KEY").await.unwrap();
    assert_eq!(val.as_deref(), Some("sk-abc"));
}

#[tokio::test]
async fn env_fetch_falls_back_to_inbound_bearer_when_no_api_key() {
    let server = MockServer::start().await;
    let leash = client_with_bearer_only(&server, "user_jwt").with_platform_url(server.uri());

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/SOMETHING"))
        .and(header("Authorization", "Bearer user_jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "ok" })))
        .mount(&server)
        .await;

    let val = leash.env().get("SOMETHING").await.unwrap();
    assert_eq!(val.as_deref(), Some("ok"));
}

#[tokio::test]
async fn env_get_returns_none_for_404() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/MISSING"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": "not declared" })))
        .mount(&server)
        .await;

    assert!(leash.env().get("MISSING").await.unwrap().is_none());
}

#[tokio::test]
async fn env_get_caches_value_for_subsequent_reads() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/CACHED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "v1" })))
        .expect(1)
        .mount(&server)
        .await;

    let env = leash.env();
    assert_eq!(env.get("CACHED").await.unwrap().as_deref(), Some("v1"));
    assert_eq!(env.get("CACHED").await.unwrap().as_deref(), Some("v1"));
}

#[tokio::test]
async fn env_get_fresh_bypasses_cache() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/FRESH"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "v1" })))
        .expect(2)
        .mount(&server)
        .await;

    let env = leash.env();
    let _ = env.get("FRESH").await.unwrap();
    let _ = env.get_fresh("FRESH").await.unwrap();
}

#[tokio::test]
async fn env_get_many_shares_cache() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/K1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "v1" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/K2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "value": "v2" })))
        .expect(1)
        .mount(&server)
        .await;

    let env = leash.env();
    let many = env.get_many(&["K1", "K2"]).await.unwrap();
    assert_eq!(many.get("K1").unwrap().as_deref(), Some("v1"));
    assert_eq!(many.get("K2").unwrap().as_deref(), Some("v2"));
}

#[tokio::test]
async fn env_get_propagates_402_as_upgrade_required() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/PAY"))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({ "requiredPlan": "growth" })))
        .mount(&server)
        .await;

    let err = leash.env().get("PAY").await.unwrap_err();
    assert!(err.is_upgrade_required());
    assert!(err.to_string().contains("growth"));
}

#[tokio::test]
async fn env_get_propagates_401_as_unauthorized() {
    let server = MockServer::start().await;
    let leash = client_with_key(&server, "k");

    Mock::given(method("GET"))
        .and(path("/api/apps/me/secrets/NOPE"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let err = leash.env().get("NOPE").await.unwrap_err();
    assert!(err.is_unauthorized());
}

#[tokio::test]
async fn env_get_without_credential_errors() {
    std::env::remove_var("LEASH_API_KEY");
    let server = MockServer::start().await;
    let req = http::Request::builder().body(()).unwrap();
    let leash = Leash::new(&req).unwrap().with_platform_url(server.uri());
    let err = leash.env().get("ANYTHING").await.unwrap_err();
    assert!(err.is_unauthorized());
}

// ---------------------------------------------------------------------------
// Gmail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn gmail_list_messages_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .and(body_partial_json(json!({ "maxResults": 5 })))
        .respond_with(ok_envelope(json!({
            "messages": [{ "id": "m1", "threadId": "t1" }],
            "resultSizeEstimate": 1
        })))
        .mount(&server)
        .await;

    let res = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams {
            max_results: Some(5),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(res.messages[0].id, "m1");
}

#[tokio::test]
async fn gmail_get_message_passes_format() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/get-message"))
        .and(body_json(json!({ "messageId": "m1", "format": "metadata" })))
        .respond_with(ok_envelope(json!({ "id": "m1" })))
        .mount(&server)
        .await;

    let res = leash
        .integrations()
        .gmail()
        .get_message("m1", Some(GmailMessageFormat::Metadata))
        .await
        .unwrap();
    assert_eq!(res.get("id").and_then(|v| v.as_str()), Some("m1"));
}

#[tokio::test]
async fn gmail_send_message_round_trips() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/send-message"))
        .and(body_partial_json(json!({ "to": "x@y" })))
        .respond_with(ok_envelope(json!({ "id": "out" })))
        .mount(&server)
        .await;

    let _ = leash
        .integrations()
        .gmail()
        .send_message(GmailSendMessageParams {
            to: "x@y".into(),
            subject: "hi".into(),
            body: "msg".into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn gmail_search_messages_skips_optional_max() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/search-messages"))
        .and(body_json(json!({ "query": "from:x" })))
        .respond_with(ok_envelope(json!({ "messages": [] })))
        .mount(&server)
        .await;

    let res = leash
        .integrations()
        .gmail()
        .search_messages("from:x", None)
        .await
        .unwrap();
    assert!(res.messages.is_empty());
}

#[tokio::test]
async fn gmail_list_labels_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-labels"))
        .respond_with(ok_envelope(
            json!({ "labels": [{ "id": "INBOX", "name": "Inbox", "type": "system" }] }),
        ))
        .mount(&server)
        .await;

    let labels = leash.integrations().gmail().list_labels().await.unwrap();
    assert_eq!(labels.labels.len(), 1);
    assert_eq!(labels.labels[0].id, "INBOX");
}

#[tokio::test]
async fn gmail_get_profile_returns_raw() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/get-profile"))
        .respond_with(ok_envelope(json!({ "emailAddress": "u@e" })))
        .mount(&server)
        .await;

    let res = leash.integrations().gmail().get_profile().await.unwrap();
    assert_eq!(res.get("emailAddress").and_then(|v| v.as_str()), Some("u@e"));
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

#[tokio::test]
async fn calendar_list_calendars_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_calendar/list-calendars"))
        .respond_with(ok_envelope(json!({
            "calendars": [{ "id": "primary", "summary": "Primary" }]
        })))
        .mount(&server)
        .await;
    let res = leash.integrations().calendar().list_calendars().await.unwrap();
    assert_eq!(res.calendars[0].id, "primary");
}

#[tokio::test]
async fn calendar_list_events_with_params() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_calendar/list-events"))
        .and(body_partial_json(json!({ "maxResults": 3 })))
        .respond_with(ok_envelope(json!({ "events": [] })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .calendar()
        .list_events(CalendarListEventsParams {
            max_results: Some(3),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(res.events.is_empty());
}

#[tokio::test]
async fn calendar_create_event_round_trips() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_calendar/create-event"))
        .and(body_partial_json(json!({ "summary": "Sync" })))
        .respond_with(ok_envelope(json!({ "id": "ev1", "summary": "Sync" })))
        .mount(&server)
        .await;
    let event = leash
        .integrations()
        .calendar()
        .create_event(CalendarCreateEventParams {
            summary: "Sync".into(),
            start: EventDateTime {
                date_time: Some("2026-01-01T10:00:00Z".into()),
                ..Default::default()
            },
            end: EventDateTime {
                date_time: Some("2026-01-01T11:00:00Z".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(event.id, "ev1");
}

#[tokio::test]
async fn calendar_get_event_with_calendar_id() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_calendar/get-event"))
        .and(body_json(json!({ "eventId": "e1", "calendarId": "primary" })))
        .respond_with(ok_envelope(json!({ "id": "e1" })))
        .mount(&server)
        .await;
    let event = leash
        .integrations()
        .calendar()
        .get_event("e1", Some("primary"))
        .await
        .unwrap();
    assert_eq!(event.id, "e1");
}

#[tokio::test]
async fn calendar_aliases_match_canonical() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_calendar/list-calendars"))
        .respond_with(ok_envelope(json!({ "calendars": [] })))
        .expect(2)
        .mount(&server)
        .await;
    let _ = leash.integrations().calendar().list_calendars().await.unwrap();
    let _ = leash
        .integrations()
        .google_calendar()
        .list_calendars()
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Drive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drive_list_files_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/list-files"))
        .respond_with(ok_envelope(json!({
            "files": [{ "id": "f1", "name": "doc", "mimeType": "text/plain" }]
        })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .drive()
        .list_files(DriveListFilesParams::default())
        .await
        .unwrap();
    assert_eq!(res.files[0].id, "f1");
}

#[tokio::test]
async fn drive_get_file_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/get-file"))
        .and(body_json(json!({ "fileId": "f1" })))
        .respond_with(ok_envelope(
            json!({ "id": "f1", "name": "doc", "mimeType": "text/plain" }),
        ))
        .mount(&server)
        .await;
    let file = leash.integrations().drive().get_file("f1").await.unwrap();
    assert_eq!(file.name, "doc");
}

#[tokio::test]
async fn drive_download_file_returns_raw() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/download-file"))
        .respond_with(ok_envelope(json!({ "bytes": "aGVsbG8=" })))
        .mount(&server)
        .await;
    let res = leash.integrations().drive().download_file("f1").await.unwrap();
    assert_eq!(res.get("bytes").and_then(|v| v.as_str()), Some("aGVsbG8="));
}

#[tokio::test]
async fn drive_create_folder_passes_parent() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/create-folder"))
        .and(body_json(json!({ "name": "New", "parentId": "p1" })))
        .respond_with(ok_envelope(json!({ "id": "f2", "name": "New", "mimeType": "application/vnd.google-apps.folder" })))
        .mount(&server)
        .await;
    let _ = leash
        .integrations()
        .drive()
        .create_folder("New", Some("p1"))
        .await
        .unwrap();
}

#[tokio::test]
async fn drive_upload_file_round_trips() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/upload-file"))
        .and(body_partial_json(json!({ "name": "x.txt", "mimeType": "text/plain" })))
        .respond_with(ok_envelope(json!({ "id": "f3", "name": "x.txt", "mimeType": "text/plain" })))
        .mount(&server)
        .await;
    let _ = leash
        .integrations()
        .drive()
        .upload_file(DriveUploadFileParams {
            name: "x.txt".into(),
            content: "hello".into(),
            mime_type: "text/plain".into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn drive_delete_file_returns_raw() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/delete-file"))
        .respond_with(ok_envelope(json!({ "ok": true })))
        .mount(&server)
        .await;
    let _ = leash.integrations().drive().delete_file("f1").await.unwrap();
}

#[tokio::test]
async fn drive_search_files_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/search-files"))
        .and(body_partial_json(json!({ "query": "foo" })))
        .respond_with(ok_envelope(json!({ "files": [] })))
        .mount(&server)
        .await;
    let _ = leash
        .integrations()
        .drive()
        .search_files("foo", Some(10))
        .await
        .unwrap();
}

#[tokio::test]
async fn drive_aliases_match_canonical() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/google_drive/list-files"))
        .respond_with(ok_envelope(json!({ "files": [] })))
        .expect(2)
        .mount(&server)
        .await;
    let _ = leash
        .integrations()
        .drive()
        .list_files(DriveListFilesParams::default())
        .await
        .unwrap();
    let _ = leash
        .integrations()
        .google_drive()
        .list_files(DriveListFilesParams::default())
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Linear
// ---------------------------------------------------------------------------

#[tokio::test]
async fn linear_list_issues_envelope() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_issues"))
        .respond_with(ok_envelope(json!({
            "issues": [{ "id": "iss1", "title": "T", "identifier": "LEA-1" }],
            "cursor": "next"
        })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .linear()
        .list_issues(LinearListIssuesFilter::default())
        .await
        .unwrap();
    assert_eq!(res.issues[0].title, "T");
    assert_eq!(res.cursor.as_deref(), Some("next"));
}

#[tokio::test]
async fn linear_list_issues_bare_array() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_issues"))
        .respond_with(ok_envelope(json!([
            { "id": "iss1", "title": "T", "identifier": "LEA-1" }
        ])))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .linear()
        .list_issues(LinearListIssuesFilter::default())
        .await
        .unwrap();
    assert_eq!(res.issues.len(), 1);
    assert!(res.cursor.is_none());
}

#[tokio::test]
async fn linear_get_issue_decodes() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/get_issue"))
        .and(body_json(json!({ "id": "u1" })))
        .respond_with(ok_envelope(json!({ "id": "u1", "title": "X" })))
        .mount(&server)
        .await;
    let res = leash.integrations().linear().get_issue("u1").await.unwrap();
    assert_eq!(res.title, "X");
}

#[tokio::test]
async fn linear_create_issue() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/create_issue"))
        .and(body_partial_json(json!({ "teamId": "T", "title": "go" })))
        .respond_with(ok_envelope(json!({ "id": "x", "title": "go" })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .linear()
        .create_issue(LinearCreateIssueInput {
            team_id: "T".into(),
            title: "go".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(res.id, "x");
}

#[tokio::test]
async fn linear_update_issue_merges_id() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/update_issue"))
        .and(body_partial_json(json!({ "id": "i1", "title": "renamed" })))
        .respond_with(ok_envelope(json!({ "id": "i1", "title": "renamed" })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .linear()
        .update_issue(
            "i1",
            LinearUpdateIssuePatch {
                title: Some("renamed".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(res.title, "renamed");
}

#[tokio::test]
async fn linear_add_comment() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/add_comment"))
        .and(body_json(json!({ "issueId": "i1", "body": "hi" })))
        .respond_with(ok_envelope(json!({ "id": "c1", "body": "hi", "issueId": "i1" })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .linear()
        .add_comment("i1", "hi")
        .await
        .unwrap();
    assert_eq!(res.body, "hi");
}

#[tokio::test]
async fn linear_list_teams_envelope() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_teams"))
        .respond_with(ok_envelope(
            json!({ "teams": [{ "id": "t1", "key": "K", "name": "Team" }] }),
        ))
        .mount(&server)
        .await;
    let teams = leash.integrations().linear().list_teams().await.unwrap();
    assert_eq!(teams.len(), 1);
}

#[tokio::test]
async fn linear_list_teams_bare_array() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_teams"))
        .respond_with(ok_envelope(json!([
            { "id": "t1", "key": "K", "name": "Team" }
        ])))
        .mount(&server)
        .await;
    let teams = leash.integrations().linear().list_teams().await.unwrap();
    assert_eq!(teams.len(), 1);
}

#[tokio::test]
async fn linear_list_projects_envelope() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_projects"))
        .respond_with(ok_envelope(
            json!({ "projects": [{ "id": "p1", "name": "Pj" }] }),
        ))
        .mount(&server)
        .await;
    let projects = leash
        .integrations()
        .linear()
        .list_projects(LinearListProjectsFilter::default())
        .await
        .unwrap();
    assert_eq!(projects.len(), 1);
}

// ---------------------------------------------------------------------------
// Generic provider escape hatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_call_round_trips() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/slack/post_message"))
        .and(body_json(json!({ "channel": "#general", "text": "hi" })))
        .respond_with(ok_envelope(json!({ "ok": true })))
        .mount(&server)
        .await;
    let res = leash
        .integrations()
        .provider("slack")
        .call("post_message", json!({ "channel": "#general", "text": "hi" }))
        .await
        .unwrap();
    assert_eq!(res.get("ok").and_then(|v| v.as_bool()), Some(true));
}

#[tokio::test]
async fn provider_name_is_exposed() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    let p = leash.integrations().provider("github");
    assert_eq!(p.name(), "github");
}

// ---------------------------------------------------------------------------
// Error mapping on integration calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_401_maps_to_unauthorized() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "error": "bad cookie" })))
        .mount(&server)
        .await;
    let err = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap_err();
    assert!(err.is_unauthorized());
}

#[tokio::test]
async fn integration_402_maps_to_plan_block_with_required_plan() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/linear/list_issues"))
        .respond_with(
            ResponseTemplate::new(402).set_body_json(json!({
                "message": "Upgrade to Growth.", "requiredPlan": "growth"
            })),
        )
        .mount(&server)
        .await;
    let err = leash
        .integrations()
        .linear()
        .list_issues(LinearListIssuesFilter::default())
        .await
        .unwrap_err();
    assert!(err.is_plan_block());
    match err {
        LeashError::PlanBlock { required_plan, .. } => {
            assert_eq!(required_plan.as_deref(), Some("growth"));
        }
        _ => panic!("expected PlanBlock"),
    }
}

#[tokio::test]
async fn integration_403_maps_to_connection_required() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({
                "error": "not connected", "connectUrl": "https://leash.build/connect"
            })),
        )
        .mount(&server)
        .await;
    let err = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap_err();
    assert!(err.is_connection_required());
    if let LeashError::ConnectionRequired {
        provider,
        connect_url,
        ..
    } = err
    {
        assert_eq!(provider, "gmail");
        assert_eq!(
            connect_url.as_deref(),
            Some("https://leash.build/connect")
        );
    } else {
        panic!("expected ConnectionRequired");
    }
}

#[tokio::test]
async fn integration_500_maps_to_upstream_error() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "error": "boom" })))
        .mount(&server)
        .await;
    let err = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(500));
    matches!(err, LeashError::UpstreamError { .. });
}

#[tokio::test]
async fn integration_envelope_success_false_surfaces() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": false, "error": "internal", "code": "UPGRADE_REQUIRED"
        })))
        .mount(&server)
        .await;
    let err = leash
        .integrations()
        .gmail()
        .list_messages(GmailListParams::default())
        .await
        .unwrap_err();
    assert!(err.is_plan_block());
}

#[tokio::test]
async fn integration_envelope_no_data_returns_raw() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/get-profile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "emailAddress": "x" })))
        .mount(&server)
        .await;
    let res = leash.integrations().gmail().get_profile().await.unwrap();
    assert_eq!(res.get("emailAddress").and_then(|v| v.as_str()), Some("x"));
}

// ---------------------------------------------------------------------------
// Required-header presence — sanity on every call
// ---------------------------------------------------------------------------

#[tokio::test]
async fn x_api_key_header_required_on_every_integration_post() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "lsk_p", "cookie");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-labels"))
        .and(header_exists("X-API-Key"))
        .respond_with(ok_envelope(json!({ "labels": [] })))
        .mount(&server)
        .await;
    let _ = leash.integrations().gmail().list_labels().await.unwrap();
}

#[tokio::test]
async fn cookie_header_forwarded_on_every_integration_post() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "cookie-val");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-labels"))
        .and(header_exists("Cookie"))
        .respond_with(ok_envelope(json!({ "labels": [] })))
        .mount(&server)
        .await;
    let _ = leash.integrations().gmail().list_labels().await.unwrap();
}

// ---------------------------------------------------------------------------
// Server-only check that body shape uses JSON content-type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_call_sets_json_content_type() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");
    Mock::given(method("POST"))
        .and(path("/api/integrations/gmail/list-labels"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ok_envelope(json!({ "labels": [] })))
        .mount(&server)
        .await;
    let _ = leash.integrations().gmail().list_labels().await.unwrap();
}

// ---------------------------------------------------------------------------
// Identity reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_user_returns_none_when_no_cookie() {
    std::env::remove_var("LEASH_API_KEY");
    let req = http::Request::builder().body(()).unwrap();
    let leash = Leash::new(&req).unwrap();
    assert!(leash.auth().user().await.unwrap().is_none());
    assert!(!leash.auth().is_authenticated().await.unwrap());
}

// ---------------------------------------------------------------------------
// Quick sanity: ensure we don't construct the request capturing helper
// ---------------------------------------------------------------------------

#[tokio::test]
async fn captured_request_was_not_mutated_after_construct() {
    let req = http::Request::builder()
        .header("cookie", "leash-auth=abc")
        .body(())
        .unwrap();
    let leash = Leash::new(&req).unwrap();
    let _ = leash; // dropping is fine
}

// We also wanted to lightly exercise unused helper to keep coverage honest.
#[tokio::test]
async fn provider_call_uses_correct_path() {
    let server = MockServer::start().await;
    let leash = client_with_key_and_cookie(&server, "k", "c");

    let received: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let received_clone = received.clone();
    Mock::given(method("POST"))
        .and(path("/api/integrations/foo-provider/some_action"))
        .respond_with(move |req: &Request| {
            received_clone.lock().unwrap().push(req.url.path().to_string());
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": {} }))
        })
        .mount(&server)
        .await;
    let _ = leash
        .integrations()
        .provider("foo-provider")
        .call("some_action", json!({}))
        .await
        .unwrap();
    let captured = received.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], "/api/integrations/foo-provider/some_action");
}
