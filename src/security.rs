//! Security utilities: secure string handling and key validation.

use std::fmt;

/// A string that masks its contents in `Debug` and `Display` outputs.
///
/// Useful for secrets like API keys that should not appear in logs.
#[derive(Clone)]
pub struct SecureString(String);

impl SecureString {
    /// Wraps a value into a secure string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the inner value as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper, returning the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl fmt::Display for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl From<String> for SecureString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecureString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// Validates a golden key format.
///
/// Returns `Ok(())` if the key is non-empty, at least 8 characters,
/// and contains only alphanumeric characters, underscores, and hyphens.
pub fn validate_golden_key(key: &str) -> Result<(), &'static str> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("golden key must not be empty");
    }
    if trimmed.len() < 8 {
        return Err("golden key must be at least 8 characters");
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err("golden key contains invalid characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_string_masks_content() {
        let s = SecureString::new("my-secret-key");
        assert_eq!(format!("{s:?}"), "***");
        assert_eq!(format!("{s}"), "***");
        assert_eq!(s.as_str(), "my-secret-key");
        assert_eq!(s.into_inner(), "my-secret-key");
    }

    #[test]
    fn validates_golden_key() {
        assert!(validate_golden_key("abc12345").is_ok());
        assert!(validate_golden_key("abc_123-def").is_ok());
        assert!(validate_golden_key("").is_err());
        assert!(validate_golden_key("short").is_err());
        assert!(validate_golden_key("invalid chars!").is_err());
    }
}
