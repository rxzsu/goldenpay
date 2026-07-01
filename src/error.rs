//! Error types for the goldenpay SDK.

use thiserror::Error;

/// Errors returned by the goldenpay SDK.
#[derive(Debug, Error)]
pub enum GoldenPayError {
    /// The golden key was empty or missing.
    #[error("missing golden key")]
    MissingGoldenKey,
    /// Authentication failed; the golden key or session is invalid.
    #[error("unauthorized")]
    Unauthorized,
    /// An HTTP transport error occurred.
    #[error("http error: {source}")]
    Http {
        #[from]
        source: reqwest::Error,
    },
    /// A JSON serialization or deserialization error.
    #[error("json error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    /// An I/O error (file read/write, etc.).
    #[error("io error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    /// Failed to parse HTML or API response.
    #[error("parse error in {context}: {message}")]
    Parse {
        context: &'static str,
        message: String,
    },
    /// An HTTP request received a non-success status code.
    #[error("request failed: {method} {url} -> {status}: {body}")]
    RequestFailed {
        method: &'static str,
        url: String,
        status: u16,
        body: String,
    },
    /// A delivery operation failed (out of stock, already delivered, etc.).
    #[error("delivery error: {0}")]
    Delivery(#[from] crate::automation::DeliveryError),
    /// A state store operation failed.
    #[error("state store error: {message}")]
    State { message: String },
}

impl GoldenPayError {
    /// Creates a [`Parse`](GoldenPayError::Parse) error with a context label.
    pub fn parse(context: &'static str, message: impl Into<String>) -> Self {
        Self::Parse {
            context,
            message: message.into(),
        }
    }

    /// Creates a [`State`](GoldenPayError::State) error with a descriptive message.
    pub fn state(message: impl Into<String>) -> Self {
        Self::State {
            message: message.into(),
        }
    }
}
