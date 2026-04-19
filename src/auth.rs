//! Framework-agnostic server authentication for the Leash platform.
//!
//! Extracts the authenticated user from the `leash-auth` cookie without
//! depending on any specific web framework.  Works with actix-web, axum,
//! rocket, warp, or any framework that can give you the raw `Cookie` header
//! value.
//!
//! # Example (axum)
//!
//! ```no_run
//! use leash_sdk::auth::get_leash_user;
//!
//! // In an axum handler:
//! // let cookie_header = headers.get("cookie").map(|v| v.to_str().unwrap());
//! // let user = get_leash_user(cookie_header.unwrap_or(""))?;
//! ```

use crate::types::LeashError;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// The cookie name set by the Leash platform.
const LEASH_AUTH_COOKIE: &str = "leash-auth";

/// Authenticated user extracted from a Leash JWT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LeashUser {
    /// Unique user identifier.
    #[serde(rename = "userId")]
    pub id: String,
    /// User email address.
    pub email: String,
    /// Display name.
    pub name: String,
    /// Profile picture URL, if available.
    #[serde(default)]
    pub picture: Option<String>,
}

/// JWT claims — a superset of [`LeashUser`] that includes standard JWT fields.
#[derive(Debug, Deserialize)]
struct Claims {
    #[serde(rename = "userId")]
    user_id: String,
    email: String,
    name: String,
    #[serde(default)]
    picture: Option<String>,
}

impl From<Claims> for LeashUser {
    fn from(c: Claims) -> Self {
        Self {
            id: c.user_id,
            email: c.email,
            name: c.name,
            picture: c.picture,
        }
    }
}

/// Extract the authenticated [`LeashUser`] from a raw `Cookie` header string.
///
/// Parses the header to find the `leash-auth` cookie, then decodes the JWT.
/// If the `LEASH_JWT_SECRET` environment variable is set the signature is
/// verified; otherwise the token is decoded without verification.
///
/// # Errors
///
/// Returns [`LeashError::ApiError`] if:
/// - The `leash-auth` cookie is not present in the header.
/// - The JWT cannot be decoded or verified.
pub fn get_leash_user(cookie_header: &str) -> Result<LeashUser, LeashError> {
    let token = parse_cookie(cookie_header, LEASH_AUTH_COOKIE).ok_or_else(|| {
        LeashError::ApiError {
            message: "leash-auth cookie not found".to_string(),
            code: Some("missing_cookie".to_string()),
        }
    })?;

    get_leash_user_from_cookie(token)
}

/// Decode a [`LeashUser`] directly from a raw JWT token string.
///
/// This is useful when your framework has already parsed the cookies for you
/// and you have the token value in hand.
///
/// If the `LEASH_JWT_SECRET` environment variable is set the signature is
/// verified; otherwise the token is decoded without verification.
///
/// # Errors
///
/// Returns [`LeashError::ApiError`] if the JWT cannot be decoded or verified.
pub fn get_leash_user_from_cookie(token: &str) -> Result<LeashUser, LeashError> {
    let claims = decode_jwt(token)?;
    Ok(LeashUser::from(claims))
}

/// Check whether the raw `Cookie` header contains a valid `leash-auth` token.
///
/// Returns `true` when [`get_leash_user`] would succeed, `false` otherwise.
pub fn is_authenticated(cookie_header: &str) -> bool {
    get_leash_user(cookie_header).is_ok()
}

/// Check whether a raw JWT token string represents a valid Leash session.
///
/// Returns `true` when [`get_leash_user_from_cookie`] would succeed, `false`
/// otherwise.
pub fn is_authenticated_from_cookie(token: &str) -> bool {
    get_leash_user_from_cookie(token).is_ok()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Find a cookie value by name in a raw `Cookie` header string.
fn parse_cookie<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    for pair in header.split(';') {
        let pair = pair.trim();
        if let Some(rest) = pair.strip_prefix(name) {
            if let Some(value) = rest.strip_prefix('=') {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Decode (and optionally verify) a JWT token into [`Claims`].
fn decode_jwt(token: &str) -> Result<Claims, LeashError> {
    match std::env::var("LEASH_JWT_SECRET") {
        Ok(secret) if !secret.is_empty() => {
            // Verify signature with the configured secret.
            let key = DecodingKey::from_secret(secret.as_bytes());
            let mut validation = Validation::new(Algorithm::HS256);
            // The platform tokens may not always carry exp; be lenient.
            validation.required_spec_claims.clear();
            validation.validate_exp = false;

            let data = decode::<Claims>(token, &key, &validation).map_err(|e| {
                LeashError::ApiError {
                    message: format!("JWT verification failed: {e}"),
                    code: Some("invalid_token".to_string()),
                }
            })?;
            Ok(data.claims)
        }
        _ => {
            // No secret configured — decode without verification.
            let mut validation = Validation::new(Algorithm::HS256);
            validation.insecure_disable_signature_validation();
            validation.required_spec_claims.clear();
            validation.validate_exp = false;

            let key = DecodingKey::from_secret(b"");
            let data = decode::<Claims>(token, &key, &validation).map_err(|e| {
                LeashError::ApiError {
                    message: format!("JWT decode failed: {e}"),
                    code: Some("invalid_token".to_string()),
                }
            })?;
            Ok(data.claims)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestClaims {
        #[serde(rename = "userId")]
        user_id: String,
        email: String,
        name: String,
        picture: Option<String>,
    }

    fn make_token(claims: &TestClaims, secret: &str) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    fn sample_claims() -> TestClaims {
        TestClaims {
            user_id: "usr_123".to_string(),
            email: "alice@example.com".to_string(),
            name: "Alice".to_string(),
            picture: Some("https://img.example.com/alice.png".to_string()),
        }
    }

    #[test]
    fn valid_token_returns_user() {
        // Ensure no secret so we use insecure decode.
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = sample_claims();
        let token = make_token(&claims, "any-secret");
        let header = format!("other=foo; leash-auth={token}; session=bar");

        let user = get_leash_user(&header).unwrap();
        assert_eq!(user.id, "usr_123");
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.name, "Alice");
        assert_eq!(
            user.picture,
            Some("https://img.example.com/alice.png".to_string())
        );
    }

    #[test]
    fn missing_cookie_returns_error() {
        let header = "session=abc; other=xyz";
        let err = get_leash_user(header).unwrap_err();
        assert!(err.to_string().contains("leash-auth cookie not found"));
    }

    #[test]
    fn empty_header_returns_error() {
        let err = get_leash_user("").unwrap_err();
        assert!(err.to_string().contains("leash-auth cookie not found"));
    }

    #[test]
    fn invalid_token_returns_error() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let header = "leash-auth=not-a-jwt";
        let err = get_leash_user(header).unwrap_err();
        assert!(err.to_string().contains("JWT decode failed"));
    }

    #[test]
    fn no_secret_decodes_without_verification() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = sample_claims();
        // Sign with an arbitrary secret — should still decode fine
        // when LEASH_JWT_SECRET is not set.
        let token = make_token(&claims, "some-random-secret");
        let user = get_leash_user_from_cookie(&token).unwrap();
        assert_eq!(user.id, "usr_123");
        assert_eq!(user.email, "alice@example.com");
    }

    #[test]
    fn with_secret_verifies_signature() {
        let secret = "test-secret-key";
        std::env::set_var("LEASH_JWT_SECRET", secret);

        let claims = sample_claims();
        let token = make_token(&claims, secret);
        let user = get_leash_user_from_cookie(&token).unwrap();
        assert_eq!(user.id, "usr_123");

        // Clean up.
        std::env::remove_var("LEASH_JWT_SECRET");
    }

    #[test]
    fn with_secret_rejects_wrong_signature() {
        let secret = "correct-secret";
        std::env::set_var("LEASH_JWT_SECRET", secret);

        let claims = sample_claims();
        let token = make_token(&claims, "wrong-secret");
        let err = get_leash_user_from_cookie(&token).unwrap_err();
        assert!(err.to_string().contains("JWT verification failed"));

        // Clean up.
        std::env::remove_var("LEASH_JWT_SECRET");
    }

    #[test]
    fn get_leash_user_from_cookie_works_with_raw_token() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = sample_claims();
        let token = make_token(&claims, "secret");
        let user = get_leash_user_from_cookie(&token).unwrap();
        assert_eq!(user.id, "usr_123");
        assert_eq!(user.name, "Alice");
    }

    #[test]
    fn picture_is_optional() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = TestClaims {
            user_id: "usr_456".to_string(),
            email: "bob@example.com".to_string(),
            name: "Bob".to_string(),
            picture: None,
        };
        let token = make_token(&claims, "s");
        let user = get_leash_user_from_cookie(&token).unwrap();
        assert_eq!(user.id, "usr_456");
        assert_eq!(user.picture, None);
    }

    #[test]
    fn is_authenticated_returns_true_for_valid_cookie() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = sample_claims();
        let token = make_token(&claims, "any-secret");
        let header = format!("leash-auth={token}");

        assert!(is_authenticated(&header));
    }

    #[test]
    fn is_authenticated_returns_false_for_missing_cookie() {
        assert!(!is_authenticated("session=abc"));
    }

    #[test]
    fn is_authenticated_from_cookie_returns_true_for_valid_token() {
        std::env::remove_var("LEASH_JWT_SECRET");

        let claims = sample_claims();
        let token = make_token(&claims, "any-secret");

        assert!(is_authenticated_from_cookie(&token));
    }

    #[test]
    fn is_authenticated_from_cookie_returns_false_for_invalid_token() {
        std::env::remove_var("LEASH_JWT_SECRET");

        assert!(!is_authenticated_from_cookie("not-a-jwt"));
    }

    #[test]
    fn parse_cookie_handles_edge_cases() {
        // Cookie is the first in the header.
        assert_eq!(parse_cookie("leash-auth=tok123", "leash-auth"), Some("tok123"));
        // Cookie has spaces around semicolons.
        assert_eq!(
            parse_cookie("a=1 ; leash-auth=tok123 ; b=2", "leash-auth"),
            Some("tok123")
        );
        // No match.
        assert_eq!(parse_cookie("other=val", "leash-auth"), None);
        // Similar prefix should not match.
        assert_eq!(parse_cookie("leash-auth-extra=val", "leash-auth"), None);
    }
}
