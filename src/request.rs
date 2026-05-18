//! Framework-agnostic request abstraction.
//!
//! Implementing [`LeashRequest`] for a custom request type takes two
//! lookups — cookie by name and header by name — and gives the constructor
//! everything it needs. Blanket impls cover the common frameworks: plain
//! [`http::Request`], `axum::extract::Request` (gated behind the `axum`
//! feature), and `actix_web::HttpRequest` (`actix-web` feature).

/// Any HTTP request the SDK can read auth context off.
///
/// The SDK only needs two operations:
///
///   * [`cookie`](Self::cookie) — read a named cookie (e.g. `leash-auth`).
///   * [`header`](Self::header) — read a named header (e.g. `Authorization`).
///
/// Implementations should be lenient — never panic on a malformed cookie or
/// missing header; just return [`None`].
pub trait LeashRequest {
    /// Return the value of the named cookie, when present.
    fn cookie(&self, name: &str) -> Option<String>;

    /// Return the value of the named header (case-insensitive), when present.
    fn header(&self, name: &str) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Plain http::Request<T>
// ---------------------------------------------------------------------------

/// Look up `name` in a `Cookie:` header value.
///
/// Handles whitespace-tolerant `name=value` pairs separated by `;` — matches
/// the TS / Python / Go cookie parsers.
fn parse_cookie_header<'a>(header: &'a str, name: &str) -> Option<&'a str> {
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

fn header_value<T>(req: &http::Request<T>, name: &str) -> Option<String> {
    // http::HeaderName lookups are case-insensitive when constructed from
    // lowercase. Try the supplied name first (preserves explicit casing for
    // callers), then fall back to lowercase.
    req.headers()
        .get(name)
        .or_else(|| req.headers().get(name.to_ascii_lowercase().as_str()))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

impl<T> LeashRequest for &http::Request<T> {
    fn cookie(&self, name: &str) -> Option<String> {
        let raw = header_value(self, "cookie")?;
        parse_cookie_header(&raw, name).map(|s| s.to_string())
    }

    fn header(&self, name: &str) -> Option<String> {
        header_value(self, name)
    }
}

impl<T> LeashRequest for http::Request<T> {
    fn cookie(&self, name: &str) -> Option<String> {
        let raw = header_value(self, "cookie")?;
        parse_cookie_header(&raw, name).map(|s| s.to_string())
    }

    fn header(&self, name: &str) -> Option<String> {
        header_value(self, name)
    }
}

// ---------------------------------------------------------------------------
// http::request::Parts (handy for axum/actix extractors that hand back Parts)
// ---------------------------------------------------------------------------

impl LeashRequest for &http::request::Parts {
    fn cookie(&self, name: &str) -> Option<String> {
        let raw = self
            .headers
            .get("cookie")
            .or_else(|| self.headers.get("Cookie"))
            .and_then(|v| v.to_str().ok())?;
        parse_cookie_header(raw, name).map(|s| s.to_string())
    }

    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .or_else(|| self.headers.get(name.to_ascii_lowercase().as_str()))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }
}

impl LeashRequest for http::request::Parts {
    fn cookie(&self, name: &str) -> Option<String> {
        (&self).cookie(name)
    }

    fn header(&self, name: &str) -> Option<String> {
        (&self).header(name)
    }
}

// ---------------------------------------------------------------------------
// Headers-only (when a framework hands you just headers)
// ---------------------------------------------------------------------------

impl LeashRequest for &http::HeaderMap {
    fn cookie(&self, name: &str) -> Option<String> {
        let raw = self
            .get("cookie")
            .or_else(|| self.get("Cookie"))
            .and_then(|v| v.to_str().ok())?;
        parse_cookie_header(raw, name).map(|s| s.to_string())
    }

    fn header(&self, name: &str) -> Option<String> {
        self.get(name)
            .or_else(|| self.get(name.to_ascii_lowercase().as_str()))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }
}

// ---------------------------------------------------------------------------
// axum (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "axum")]
mod axum_impl {
    use super::*;

    // `axum::extract::Request` is just `http::Request<axum::body::Body>`.
    // The blanket impl over `http::Request<T>` already covers it — re-export
    // a doc-only note here so the symbol shows up in docs.

    /// Verifies that `&http::Request<B>` works for axum's request shape.
    ///
    /// `axum::extract::Request` is `http::Request<axum::body::Body>`, so the
    /// blanket impl on `&http::Request<T>` already applies. This helper is
    /// kept so the feature flag has a non-empty surface and so users searching
    /// for "axum" in the docs land somewhere meaningful.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn extract_with_request<B>(_req: &http::Request<B>) -> &dyn LeashRequest
    where
        for<'a> &'a http::Request<B>: LeashRequest,
    {
        // Compile-time only — return a static reference for the trait object.
        unreachable!("doc-only helper, never call directly")
    }
}

// ---------------------------------------------------------------------------
// actix-web (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "actix-web")]
mod actix_impl {
    use super::*;
    use actix_web::HttpRequest;

    impl LeashRequest for &HttpRequest {
        fn cookie(&self, name: &str) -> Option<String> {
            // Disambiguate: actix `HttpRequest::cookie(&self, &str) -> Option<Cookie>`.
            HttpRequest::cookie(self, name).map(|c| c.value().to_string())
        }

        fn header(&self, name: &str) -> Option<String> {
            self.headers()
                .get(name)
                .or_else(|| self.headers().get(name.to_ascii_lowercase().as_str()))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        }
    }

    impl LeashRequest for HttpRequest {
        fn cookie(&self, name: &str) -> Option<String> {
            HttpRequest::cookie(self, name).map(|c| c.value().to_string())
        }

        fn header(&self, name: &str) -> Option<String> {
            self.headers()
                .get(name)
                .or_else(|| self.headers().get(name.to_ascii_lowercase().as_str()))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn req_with(cookie: Option<&str>, auth: Option<&str>) -> http::Request<()> {
        let mut builder = http::Request::builder().uri("/");
        if let Some(c) = cookie {
            builder = builder.header("cookie", c);
        }
        if let Some(a) = auth {
            builder = builder.header("authorization", a);
        }
        builder.body(()).unwrap()
    }

    #[test]
    fn cookie_extraction_works() {
        let req = req_with(Some("other=foo; leash-auth=tok123; bar=baz"), None);
        assert_eq!(req.cookie("leash-auth"), Some("tok123".to_string()));
        assert_eq!(req.cookie("bar"), Some("baz".to_string()));
        assert_eq!(req.cookie("missing"), None);
    }

    #[test]
    fn cookie_returns_none_when_header_missing() {
        let req = req_with(None, None);
        assert_eq!(req.cookie("leash-auth"), None);
    }

    #[test]
    fn similar_prefix_does_not_match() {
        let req = req_with(Some("leash-auth-extra=nope"), None);
        assert_eq!(req.cookie("leash-auth"), None);
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let req = req_with(None, Some("Bearer abc.def"));
        assert_eq!(req.header("authorization"), Some("Bearer abc.def".to_string()));
        assert_eq!(req.header("Authorization"), Some("Bearer abc.def".to_string()));
    }

    #[test]
    fn parts_impl_round_trips_cookies() {
        let req = req_with(Some("leash-auth=parts"), None);
        let (parts, _) = req.into_parts();
        assert_eq!(parts.cookie("leash-auth"), Some("parts".to_string()));
    }

    #[test]
    fn headermap_impl_round_trips() {
        let req = req_with(Some("leash-auth=hdr"), Some("Bearer xyz"));
        let headers = req.headers().clone();
        assert_eq!((&headers).cookie("leash-auth"), Some("hdr".to_string()));
        assert_eq!((&headers).header("authorization"), Some("Bearer xyz".to_string()));
    }

    #[test]
    fn owned_request_implements_leash_request() {
        let req = req_with(Some("leash-auth=owned"), None);
        // Exercise the owned-value impl
        let v: &dyn LeashRequest = &req;
        assert_eq!(v.cookie("leash-auth"), Some("owned".to_string()));
    }
}
