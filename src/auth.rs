//! Identity helpers — extract the authenticated [`LeashUser`] off a JWT.
//!
//! Mirrors `leash-sdk-ts/src/server/auth.ts`, `leash-sdk-python/leash/auth.py`,
//! and `leash-sdk-go/auth.go`. JWT parsing is stdlib-only (no `jsonwebtoken`
//! dependency) so the SDK stays slim and avoids OpenSSL transitively.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::errors::{LeashError, Result};
use crate::request::LeashRequest;

/// The cookie name set by the Leash platform.
pub const LEASH_AUTH_COOKIE: &str = "leash-auth";

/// Authenticated user extracted from a Leash JWT.
///
/// Returned by [`Auth::user`](crate::Auth::user) and the standalone
/// [`get_leash_user`] helper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeashUser {
    /// Unique user identifier (`userId` claim, with `sub` fallback).
    pub id: String,
    /// Email address.
    pub email: String,
    /// Display name.
    pub name: String,
    /// Profile picture URL, when present in the JWT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

/// Raw JWT claims set by the platform on the `leash-auth` cookie.
#[derive(Debug, Default, Deserialize)]
struct Claims {
    #[serde(rename = "userId", default)]
    user_id: Option<String>,
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    picture: Option<String>,
    #[serde(default)]
    exp: Option<i64>,
}

impl Claims {
    fn into_user(self) -> Result<LeashUser> {
        let id = self
            .user_id
            .or(self.sub)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| LeashError::Unauthorized {
                message: "leash-auth cookie is missing a user identifier.".to_string(),
            })?;
        Ok(LeashUser {
            id,
            email: self.email.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            picture: self.picture,
        })
    }
}

/// Decode a raw `leash-auth` JWT into a [`LeashUser`].
///
/// When `LEASH_JWT_SECRET` is set, the HS256 signature is verified. Without
/// it, the SDK falls back to verify-disabled decoding so local development
/// works without provisioning the secret. The expiry claim is always
/// honoured — expired tokens are rejected regardless of signature mode.
pub fn decode_user(token: &str) -> Result<LeashUser> {
    let claims = decode_claims(token)?;
    claims.into_user()
}

/// Read the request's `leash-auth` cookie and decode it.
///
/// Errors when the cookie is missing or the JWT is invalid. Use
/// [`Auth::user`](crate::Auth::user) for a non-throwing variant.
pub fn get_leash_user<R: LeashRequest>(req: R) -> Result<LeashUser> {
    let token = req
        .cookie(LEASH_AUTH_COOKIE)
        .ok_or_else(|| LeashError::Unauthorized {
            message: "No leash-auth cookie on the request.".to_string(),
        })?;
    decode_user(&token)
}

/// `true` when [`get_leash_user`] would succeed.
pub fn is_authenticated<R: LeashRequest>(req: R) -> bool {
    get_leash_user(req).is_ok()
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

fn decode_claims(token: &str) -> Result<Claims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(LeashError::Unauthorized {
            message: "Invalid leash-auth cookie: malformed JWT.".to_string(),
        });
    }

    // Optional signature check.
    if let Ok(secret) = std::env::var("LEASH_JWT_SECRET") {
        if !secret.is_empty() {
            verify_hs256(parts[0], parts[1], parts[2], &secret)?;
        }
    }

    let payload_bytes =
        URL_SAFE_NO_PAD
            .decode(parts[1].as_bytes())
            .map_err(|_| LeashError::Unauthorized {
                message: "Invalid leash-auth cookie: payload not valid base64url.".to_string(),
            })?;

    let claims: Claims =
        serde_json::from_slice(&payload_bytes).map_err(|_| LeashError::Unauthorized {
            message: "Invalid leash-auth cookie: payload not valid JSON.".to_string(),
        })?;

    // Expiry check — even when signature verification is disabled.
    if let Some(exp) = claims.exp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if exp > 0 && now > exp {
            return Err(LeashError::Unauthorized {
                message: "leash-auth cookie has expired.".to_string(),
            });
        }
    }

    Ok(claims)
}

fn verify_hs256(header_b64: &str, payload_b64: &str, sig_b64: &str, secret: &str) -> Result<()> {
    // HS256 = HMAC-SHA256 over `header_b64.payload_b64`, signature in base64url-no-pad.
    let expected =
        URL_SAFE_NO_PAD
            .decode(sig_b64.as_bytes())
            .map_err(|_| LeashError::Unauthorized {
                message: "Invalid leash-auth cookie: signature not valid base64url.".to_string(),
            })?;

    let signing_input = format!("{header_b64}.{payload_b64}");
    let computed = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());

    if computed.len() != expected.len() {
        return Err(LeashError::Unauthorized {
            message: "Invalid leash-auth cookie: signature mismatch.".to_string(),
        });
    }
    // Constant-time compare to avoid timing leaks.
    let mut diff = 0u8;
    for (a, b) in computed.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    if diff != 0 {
        return Err(LeashError::Unauthorized {
            message: "Invalid leash-auth cookie: signature mismatch.".to_string(),
        });
    }
    Ok(())
}

// Minimal HMAC-SHA256 (RFC 2104) over SHA-256. Vendored to avoid pulling
// `hmac` + `sha2` just for one decode call — the SDK already keeps deps thin.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = sha256(key);
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Vec::with_capacity(BLOCK + msg.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(msg);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

// Tiny SHA-256 — pure Rust, no deps. Adapted from FIPS 180-4.
fn sha256(msg: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (msg.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(msg.len() + 64);
    padded.extend_from_slice(msg);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// Auth namespace — exposed off Leash::auth()
// ---------------------------------------------------------------------------

/// `leash.auth()` — non-throwing identity reads off the request that the
/// client was constructed from.
///
/// The methods are `async` so the surface aligns with the rest of the SDK and
/// stays flexible for a future remote-verify flow (today decode is purely
/// in-memory).
#[derive(Debug, Clone)]
pub struct Auth {
    pub(crate) cookie: Option<String>,
}

impl Auth {
    /// Return the authenticated user, or `None` when not authenticated.
    ///
    /// Never errors on a missing/invalid cookie — handlers can branch
    /// cleanly with `if user.is_none()`.
    pub async fn user(&self) -> Result<Option<LeashUser>> {
        let Some(cookie) = self.cookie.as_deref() else {
            return Ok(None);
        };
        match decode_user(cookie) {
            Ok(user) => Ok(Some(user)),
            Err(_) => Ok(None),
        }
    }

    /// `true` when [`Self::user`] would return `Some(_)`.
    pub async fn is_authenticated(&self) -> Result<bool> {
        Ok(self.user().await?.is_some())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    // Process-global LEASH_JWT_SECRET — serialise tests that touch it.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn b64(input: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(input)
    }

    fn make_token(payload: serde_json::Value, secret: &str) -> String {
        let header = b64(br#"{"alg":"HS256","typ":"JWT"}"#);
        let body = b64(payload.to_string().as_bytes());
        let signing_input = format!("{header}.{body}");
        let sig = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
        let sig_b64 = b64(&sig);
        format!("{header}.{body}.{sig_b64}")
    }

    #[test]
    fn decode_user_no_secret_returns_user() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        let token = make_token(
            json!({"userId":"u1","email":"a@b.c","name":"Al"}),
            "anything",
        );
        let user = decode_user(&token).unwrap();
        assert_eq!(user.id, "u1");
        assert_eq!(user.email, "a@b.c");
        assert_eq!(user.name, "Al");
        assert_eq!(user.picture, None);
    }

    #[test]
    fn decode_user_with_secret_verifies() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::set_var("LEASH_JWT_SECRET", "k");
        let token = make_token(json!({"userId":"u1","email":"a@b","name":"x"}), "k");
        let user = decode_user(&token).unwrap();
        assert_eq!(user.id, "u1");
        std::env::remove_var("LEASH_JWT_SECRET");
    }

    #[test]
    fn decode_user_rejects_bad_signature() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::set_var("LEASH_JWT_SECRET", "correct");
        let token = make_token(json!({"userId":"u1","email":"a@b","name":"x"}), "wrong");
        let err = decode_user(&token).unwrap_err();
        assert!(err.is_unauthorized());
        std::env::remove_var("LEASH_JWT_SECRET");
    }

    #[test]
    fn decode_user_rejects_expired_token() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        let token = make_token(
            json!({"userId":"u1","email":"a@b","name":"x","exp": 1}),
            "k",
        );
        let err = decode_user(&token).unwrap_err();
        assert!(err.is_unauthorized());
        assert!(matches!(err, LeashError::Unauthorized { ref message } if message.contains("expired")));
    }

    #[test]
    fn decode_user_falls_back_to_sub_when_userid_absent() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        let token = make_token(json!({"sub":"u2","email":"x@y"}), "k");
        let user = decode_user(&token).unwrap();
        assert_eq!(user.id, "u2");
        assert_eq!(user.email, "x@y");
    }

    #[test]
    fn decode_user_errors_when_no_identifier() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        let token = make_token(json!({"email":"x@y","name":"n"}), "k");
        let err = decode_user(&token).unwrap_err();
        assert!(err.is_unauthorized());
    }

    #[test]
    fn malformed_jwt_rejected() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        assert!(decode_user("not.a.jwt-tripartite").is_err());
        assert!(decode_user("only-one-part").is_err());
    }

    #[tokio::test]
    async fn auth_namespace_returns_none_when_no_cookie() {
        let auth = Auth { cookie: None };
        assert!(auth.user().await.unwrap().is_none());
        assert!(!auth.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn auth_namespace_returns_user_for_valid_cookie() {
        let auth = {
            let _g = ENV_GUARD.lock().unwrap();
            std::env::remove_var("LEASH_JWT_SECRET");
            let token = make_token(json!({"userId":"abc","email":"e","name":"n"}), "k");
            Auth { cookie: Some(token) }
            // Drop the guard before the await — the guard is non-Send and would
            // otherwise be held across .await (clippy::await_holding_lock).
        };
        let user = auth.user().await.unwrap().unwrap();
        assert_eq!(user.id, "abc");
        assert!(auth.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn auth_namespace_swallows_bad_cookie() {
        let auth = {
            let _g = ENV_GUARD.lock().unwrap();
            std::env::remove_var("LEASH_JWT_SECRET");
            Auth {
                cookie: Some("garbage".into()),
            }
        };
        // No error — just None.
        assert!(auth.user().await.unwrap().is_none());
        assert!(!auth.is_authenticated().await.unwrap());
    }

    #[test]
    fn get_leash_user_reads_from_request() {
        let _g = ENV_GUARD.lock().unwrap();
        std::env::remove_var("LEASH_JWT_SECRET");
        let token = make_token(json!({"userId":"u1","email":"e","name":"n"}), "k");
        let req = http::Request::builder()
            .header("cookie", format!("leash-auth={token}"))
            .body(())
            .unwrap();
        let user = get_leash_user(&req).unwrap();
        assert_eq!(user.id, "u1");
    }

    #[test]
    fn is_authenticated_returns_false_for_no_cookie() {
        let req = http::Request::builder().body(()).unwrap();
        assert!(!is_authenticated(&req));
    }
}
