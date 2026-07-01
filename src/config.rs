//! Configuration types: [`GoldenPayConfig`], builder, and retry policy.

use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

/// Immutable runtime configuration for the FunPay client.
///
/// Create via [`GoldenPayConfig::builder`]:
/// ```ignore
/// GoldenPayConfig::builder()
///     .golden_key("your_key")
///     .poll_interval(Duration::from_secs(3))
///     .build()
/// ```
#[derive(Clone)]
pub struct GoldenPayConfig {
    pub golden_key: String,
    pub base_url: String,
    pub user_agent: String,
    pub poll_interval: Duration,
    pub retry: RetryPolicy,
    pub max_concurrent_requests: Option<NonZeroUsize>,
    pub proxy: Option<String>,
    pub state_path: Option<PathBuf>,
}

impl fmt::Debug for GoldenPayConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoldenPayConfig")
            .field("golden_key", &"***")
            .field("base_url", &self.base_url)
            .field("user_agent", &self.user_agent)
            .field("poll_interval", &self.poll_interval)
            .field("retry", &self.retry)
            .field("max_concurrent_requests", &self.max_concurrent_requests)
            .field("proxy", &self.proxy)
            .field("state_path", &self.state_path)
            .finish()
    }
}

/// Retry configuration for failed HTTP requests.
///
/// Uses exponential backoff: `base_delay * 2^(attempt - 1)`.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct GoldenPayConfigBuilder {
    golden_key: Option<String>,
    base_url: Option<String>,
    user_agent: Option<String>,
    poll_interval: Option<Duration>,
    retry: Option<RetryPolicy>,
    max_concurrent_requests: Option<NonZeroUsize>,
    proxy: Option<String>,
    state_path: Option<PathBuf>,
}

impl GoldenPayConfig {
    /// Creates a config with the given golden key and defaults for other fields.
    pub fn new(golden_key: impl Into<String>) -> Self {
        Self {
            golden_key: golden_key.into(),
            ..Self::default()
        }
    }

    /// Returns a builder for constructing a [`GoldenPayConfig`].
    #[must_use]
    pub fn builder() -> GoldenPayConfigBuilder {
        GoldenPayConfigBuilder::default()
    }

    /// Sets an HTTP proxy for all requests.
    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Sets the file path for persistent bot state (orders and message IDs).
    pub fn with_state_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_path = Some(path.into());
        self
    }
}

impl Default for GoldenPayConfig {
    fn default() -> Self {
        Self {
            golden_key: String::new(),
            base_url: "https://funpay.com".to_string(),
            user_agent: format!("goldenpay/{}", env!("CARGO_PKG_VERSION")),
            poll_interval: Duration::from_secs(2),
            retry: RetryPolicy::default(),
            max_concurrent_requests: None,
            proxy: None,
            state_path: None,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(300),
        }
    }
}

impl RetryPolicy {
    /// Creates a retry policy with the given max attempts and base delay.
    #[must_use]
    pub fn new(max_attempts: u32, base_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
        }
    }
}

impl GoldenPayConfigBuilder {
    /// Sets the golden key used for authentication.
    pub fn golden_key(mut self, golden_key: impl Into<String>) -> Self {
        self.golden_key = Some(golden_key.into());
        self
    }

    /// Overrides the base URL (default: `https://funpay.com`).
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Overrides the User-Agent header (default: `goldenpay/{version}`).
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the delay between bot poll cycles.
    #[must_use]
    pub fn poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = Some(poll_interval);
        self
    }

    /// Sets the retry policy for failed HTTP requests.
    #[must_use]
    pub fn retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Sets an HTTP proxy for all outgoing requests.
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Sets the file path for persistent bot state.
    pub fn state_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_path = Some(path.into());
        self
    }

    /// Limits the number of concurrent API requests (default: no limit).
    #[must_use]
    pub fn max_concurrent_requests(mut self, max: NonZeroUsize) -> Self {
        self.max_concurrent_requests = Some(max);
        self
    }

    /// Consumes the builder and produces a [`GoldenPayConfig`].
    #[must_use]
    pub fn build(self) -> GoldenPayConfig {
        let defaults = GoldenPayConfig::default();
        GoldenPayConfig {
            golden_key: self.golden_key.unwrap_or(defaults.golden_key),
            base_url: self.base_url.unwrap_or(defaults.base_url),
            user_agent: self.user_agent.unwrap_or(defaults.user_agent),
            poll_interval: self.poll_interval.unwrap_or(defaults.poll_interval),
            retry: self.retry.unwrap_or(defaults.retry),
            max_concurrent_requests: self.max_concurrent_requests,
            proxy: self.proxy,
            state_path: self.state_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_overrides_defaults() {
        let config = GoldenPayConfig::builder()
            .golden_key("abc")
            .base_url("https://example.com")
            .poll_interval(Duration::from_secs(5))
            .retry_policy(RetryPolicy::new(7, Duration::from_secs(1)))
            .build();

        assert_eq!(config.golden_key, "abc");
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.poll_interval, Duration::from_secs(5));
        assert_eq!(config.retry.max_attempts, 7);
    }

    #[test]
    fn default_config_has_sane_values() {
        let config = GoldenPayConfig::default();
        assert_eq!(config.base_url, "https://funpay.com");
        assert!(config.user_agent.starts_with("goldenpay/"));
        assert_eq!(config.poll_interval, Duration::from_secs(2));
        assert!(config.proxy.is_none());
        assert!(config.state_path.is_none());
        assert_eq!(config.retry.max_attempts, 3);
    }

    #[test]
    fn default_retry_policy_is_sane() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.base_delay, Duration::from_millis(300));
    }

    #[test]
    fn config_debug_masks_golden_key() {
        let config = GoldenPayConfig::new("super-secret-key");
        let debug = format!("{config:?}");
        assert!(!debug.contains("super-secret-key"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn new_uses_default_for_omitted_fields() {
        let config = GoldenPayConfig::new("xyz");
        assert_eq!(config.golden_key, "xyz");
        assert_eq!(config.base_url, "https://funpay.com");
        assert_eq!(config.poll_interval, Duration::from_secs(2));
    }

    #[test]
    fn with_proxy_and_state_path_chaining() {
        let config = GoldenPayConfig::new("k")
            .with_proxy("http://proxy:8080")
            .with_state_path("/tmp/state.json");

        assert_eq!(config.proxy.as_deref(), Some("http://proxy:8080"));
        assert_eq!(config.state_path.as_deref().unwrap().to_str(), Some("/tmp/state.json"));
    }
}
