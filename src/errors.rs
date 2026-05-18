//! Structured error type for the Leash SDK.
//!
//! Mirrors `leash-sdk-ts/src/errors.ts`, `leash-sdk-python/leash/errors.py`,
//! and `leash-sdk-go/errors.go`. The `code` carried inside each variant is the
//! stable machine-readable identifier consumers should branch on.

/// Convenience: every async call site in the SDK returns this.
pub type Result<T> = std::result::Result<T, LeashError>;

/// The structured error returned by every Leash SDK call.
///
/// Variants are organised so the common branches (plan blocks, connection
/// required, upgrades) have first-class shape — no string parsing needed.
/// For everything else, [`LeashError::UpstreamError`] preserves the HTTP
/// status and message; [`LeashError::MalformedResponse`] covers parse
/// failures.
#[derive(Debug, thiserror::Error)]
pub enum LeashError {
    /// HTTP 402 — the caller's plan does not include this feature.
    ///
    /// The `required_plan` is best-effort: present when the platform reports
    /// it in the response body, absent otherwise.
    #[error("plan block: {message}")]
    PlanBlock {
        /// Stable code (`UPGRADE_REQUIRED` from the platform).
        code: String,
        /// Human-readable message.
        message: String,
        /// The plan tier required to unlock this call, if reported.
        required_plan: Option<String>,
    },

    /// HTTP 403 — the user hasn't connected this integration yet.
    #[error("connection required for {provider}: {message}")]
    ConnectionRequired {
        /// Provider id (e.g. `gmail`, `linear`).
        provider: String,
        /// Human-readable message.
        message: String,
        /// URL the caller can use to start the OAuth flow, when supplied.
        connect_url: Option<String>,
    },

    /// HTTP 402 surfaced from `env.get` — Growth plan or above is required.
    #[error("upgrade required: {message}")]
    UpgradeRequired {
        /// Human-readable message.
        message: String,
    },

    /// `env.get` was called for a key that isn't declared in `.env.example`
    /// or doesn't exist in any source.
    ///
    /// Note: [`crate::Env::get`] returns `Ok(None)` for this case so callers
    /// can branch with `if value.is_none()`. This variant only fires when
    /// callers reach for the lower-level surface and want the explicit error.
    #[error("env key '{key}' is not declared")]
    KeyNotDeclared {
        /// The env-var name that wasn't recognised.
        key: String,
    },

    /// HTTP 401 — missing or invalid credential.
    #[error("unauthorized: {message}")]
    Unauthorized {
        /// Human-readable message.
        message: String,
    },

    /// Failure below the HTTP layer (DNS, refused connection, TLS, etc.).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// An upstream HTTP error that doesn't map onto a more specific variant.
    #[error("upstream error (HTTP {status}): {message}")]
    UpstreamError {
        /// HTTP status code from the platform.
        status: u16,
        /// Human-readable message (often pulled from the response body).
        message: String,
    },

    /// Platform returned a response we couldn't deserialise.
    #[error("malformed response: {message}")]
    MalformedResponse {
        /// Diagnostic message including the field or shape that failed.
        message: String,
    },
}

impl LeashError {
    /// True for [`LeashError::PlanBlock`] and [`LeashError::UpgradeRequired`].
    ///
    /// Use this to render a single "upgrade your plan" UI without caring which
    /// surface (integration POST vs `env.get`) tripped the block.
    pub fn is_plan_block(&self) -> bool {
        matches!(self, Self::PlanBlock { .. } | Self::UpgradeRequired { .. })
    }

    /// Alias of [`Self::is_plan_block`] matching the TS / Go naming.
    pub fn is_upgrade_required(&self) -> bool {
        self.is_plan_block()
    }

    /// True for [`LeashError::ConnectionRequired`].
    pub fn is_connection_required(&self) -> bool {
        matches!(self, Self::ConnectionRequired { .. })
    }

    /// True for [`LeashError::Unauthorized`].
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized { .. })
    }

    /// True for [`LeashError::KeyNotDeclared`].
    pub fn is_key_not_declared(&self) -> bool {
        matches!(self, Self::KeyNotDeclared { .. })
    }

    /// True for [`LeashError::Network`].
    pub fn is_network(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    /// Returns the originating HTTP status code, when known.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::PlanBlock { .. } => Some(402),
            Self::ConnectionRequired { .. } => Some(403),
            Self::UpgradeRequired { .. } => Some(402),
            Self::Unauthorized { .. } => Some(401),
            Self::UpstreamError { status, .. } => Some(*status),
            Self::Network(e) => e.status().map(|s| s.as_u16()),
            Self::KeyNotDeclared { .. } | Self::MalformedResponse { .. } => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_block_predicate() {
        let err = LeashError::PlanBlock {
            code: "UPGRADE_REQUIRED".into(),
            message: "Growth required".into(),
            required_plan: Some("growth".into()),
        };
        assert!(err.is_plan_block());
        assert!(err.is_upgrade_required());
        assert!(!err.is_connection_required());
        assert_eq!(err.status(), Some(402));
    }

    #[test]
    fn connection_required_predicate() {
        let err = LeashError::ConnectionRequired {
            provider: "gmail".into(),
            message: "not connected".into(),
            connect_url: Some("https://leash.build/connect/gmail".into()),
        };
        assert!(err.is_connection_required());
        assert!(!err.is_plan_block());
        assert_eq!(err.status(), Some(403));
    }

    #[test]
    fn unauthorized_predicate() {
        let err = LeashError::Unauthorized {
            message: "nope".into(),
        };
        assert!(err.is_unauthorized());
        assert_eq!(err.status(), Some(401));
    }

    #[test]
    fn key_not_declared_predicate() {
        let err = LeashError::KeyNotDeclared {
            key: "OPENAI_API_KEY".into(),
        };
        assert!(err.is_key_not_declared());
        assert_eq!(err.status(), None);
    }

    #[test]
    fn upstream_error_carries_status() {
        let err = LeashError::UpstreamError {
            status: 500,
            message: "boom".into(),
        };
        assert_eq!(err.status(), Some(500));
        assert!(!err.is_plan_block());
    }

    #[test]
    fn malformed_response_has_no_status() {
        let err = LeashError::MalformedResponse {
            message: "missing field".into(),
        };
        assert_eq!(err.status(), None);
    }
}
