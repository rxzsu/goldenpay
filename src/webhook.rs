//! Webhook server for receiving FunPay notifications.
//!
//! Provides a lightweight HTTP server that accepts POST requests
//! with JSON payloads and dispatches them to a user-defined handler.
//!
//! # Example
//!
//! ```ignore
//! use goldenpay::webhook::{WebhookConfig, WebhookServer, WebhookHandler, WebhookEvent};
//!
//! struct MyHandler;
//!
//! #[async_trait::async_trait]
//! impl WebhookHandler for MyHandler {
//!     async fn handle(&self, event: WebhookEvent) -> Result<(), goldenpay::GoldenPayError> {
//!         println!("received: {:?}", event);
//!         Ok(())
//!     }
//! }
//!
//! let server = WebhookServer::new(WebhookConfig::default(), MyHandler);
//! server.run().await.unwrap();
//! ```

use crate::crypto::{hex_decode, webhook_signature, verify_hmac};
use crate::error::GoldenPayError;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Configuration for the webhook server.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Address to bind to (default: `127.0.0.1:9090`).
    pub bind_addr: SocketAddr,
    /// URL path for the webhook endpoint (default: `/webhook`).
    pub endpoint: String,
    /// Maximum body size in bytes (default: 1 MB).
    pub max_body_size: usize,
    /// Optional HMAC-SHA256 secret for verifying incoming requests.
    ///
    /// When set, the server reads the `X-Signature-256` header and
    /// rejects requests with an invalid signature (401 Unauthorized).
    pub secret: Option<String>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 9090)),
            endpoint: "/webhook".to_string(),
            max_body_size: 1_048_576,
            secret: None,
        }
    }
}

impl WebhookConfig {
    /// Sets the HMAC secret for request verification.
    #[must_use]
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }
}

/// Computes the `X-Signature-256` header value for a webhook payload.
///
/// Use this when calling a webhook endpoint that has HMAC verification enabled:
///
/// ```ignore
/// use goldenpay::webhook::compute_signature;
///
/// let body = r#"{"event":"new_order","id":"123"}"#;
/// let sig = compute_signature("my-secret", body.as_bytes());
/// // Set header: X-Signature-256: {sig}
/// ```
#[must_use]
pub fn compute_signature(secret: &str, body: &[u8]) -> String {
    webhook_signature(secret.as_bytes(), body)
}

/// A parsed webhook request with full context.
#[derive(Debug, Clone)]
pub struct WebhookPayload {
    /// Source IP address of the request.
    pub source_ip: SocketAddr,
    /// Parsed JSON body.
    pub body: serde_json::Value,
    /// HTTP request headers.
    pub headers: HashMap<String, String>,
}

/// An event received via the webhook endpoint.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum WebhookEvent {
    /// A generic JSON notification.
    Notification(WebhookPayload),
}

/// Handler trait for processing webhook events.
///
/// Implement this trait to define how your application handles
/// incoming FunPay notifications.
#[async_trait::async_trait]
pub trait WebhookHandler: Send + Sync + 'static {
    /// Called when a webhook event is received.
    async fn handle(&self, event: WebhookEvent) -> Result<(), GoldenPayError>;
}

/// A lightweight HTTP server that receives POST notifications.
pub struct WebhookServer {
    config: WebhookConfig,
    handler: Arc<dyn WebhookHandler>,
}

impl WebhookServer {
    /// Creates a new webhook server.
    pub fn new(config: WebhookConfig, handler: impl WebhookHandler) -> Self {
        Self {
            config,
            handler: Arc::new(handler),
        }
    }

    /// Starts the webhook server and runs until a fatal error.
    ///
    /// Each incoming connection is handled in a separate tokio task.
    pub async fn run(&self) -> Result<(), GoldenPayError> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        tracing::info!(
            addr = %self.config.bind_addr,
            endpoint = %self.config.endpoint,
            "webhook server started"
        );

        loop {
            let (stream, addr) = listener.accept().await?;
            let handler = self.handler.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_request(stream, addr, &config, &*handler).await {
                    tracing::warn!(%addr, error = %e, "webhook request failed");
                }
            });
        }
    }
}

async fn handle_request(
    mut stream: tokio::net::TcpStream,
    addr: SocketAddr,
    config: &WebhookConfig,
    handler: &dyn WebhookHandler,
) -> Result<(), GoldenPayError> {
    let (r, mut w) = stream.split();
    let mut reader = BufReader::new(r);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() < 2 || parts[0] != "POST" {
        send_response(&mut w, "405 Method Not Allowed", "Only POST allowed").await?;
        return Ok(());
    }

    if parts[1] != config.endpoint {
        send_response(&mut w, "404 Not Found", "Not found").await?;
        return Ok(());
    }

    let mut content_length = 0usize;
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).await?;
        if header == "\r\n" || header == "\n" {
            break;
        }
        let header = header.trim_end_matches("\r\n").trim_end_matches('\n');
        if let Some((key, value)) = header.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(key, value);
        }
    }

    if content_length == 0 || content_length > config.max_body_size {
        send_response(&mut w, "400 Bad Request", "Invalid body size").await?;
        return Ok(());
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await?;

    // HMAC verification
    if let Some(secret) = &config.secret {
        let signature_header = headers
            .get("X-Signature-256")
            .map(|s| s.as_str())
            .unwrap_or("");
        let Some(signature) = hex_decode(signature_header) else {
            send_response(&mut w, "401 Unauthorized", "Invalid signature").await?;
            return Ok(());
        };
        if !verify_hmac(secret.as_bytes(), &body, &signature) {
            send_response(&mut w, "401 Unauthorized", "Invalid signature").await?;
            return Ok(());
        }
        tracing::debug!("webhook HMAC verified");
    }

    let json_value: serde_json::Value = serde_json::from_slice(&body)?;

    let payload = WebhookPayload {
        source_ip: addr,
        body: json_value,
        headers,
    };

    handler.handle(WebhookEvent::Notification(payload)).await?;

    send_response(&mut w, "200 OK", "OK").await
}

async fn send_response(
    w: &mut tokio::net::tcp::WriteHalf<'_>,
    status: &str,
    body: &str,
) -> Result<(), GoldenPayError> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    w.write_all(response.as_bytes()).await?;
    Ok(())
}
