//! Cryptographic utilities: HMAC-SHA256 for webhook request verification.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const HEX_CHARS: &[u8] = b"0123456789abcdef";

/// Computes the HMAC-SHA256 digest of `data` using `key`.
///
/// Returns a 32-byte (256-bit) array.
#[must_use]
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Returns the hex-encoded HMAC-SHA256 of `data` under `key`.
///
/// Useful for setting the `X-Signature-256` header on webhook calls.
#[must_use]
pub fn webhook_signature(key: &[u8], data: &[u8]) -> String {
    hex_encode(&hmac_sha256(key, data))
}

/// Constant-time comparison of two byte slices.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Verifies that `signature` is a valid HMAC-SHA256 of `data` under `key`.
#[must_use]
pub fn verify_hmac(key: &[u8], data: &[u8], signature: &[u8]) -> bool {
    let expected = hmac_sha256(key, data);
    constant_time_eq(&expected, signature)
}

/// Decodes a hex string into bytes. Returns `None` on invalid input.
#[must_use]
pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            let hi = from_hex_char(s.as_bytes()[i])?;
            let lo = from_hex_char(s.as_bytes()[i + 1])?;
            Some((hi << 4) | lo)
        })
        .collect()
}

fn from_hex_char(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn hex_encode(data: &[u8]) -> String {
    let mut out = Vec::with_capacity(data.len() * 2);
    for &byte in data {
        out.push(HEX_CHARS[(byte >> 4) as usize]);
        out.push(HEX_CHARS[(byte & 0x0f) as usize]);
    }
    unsafe { String::from_utf8_unchecked(out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_deterministic() {
        let a = hmac_sha256(b"secret", b"hello");
        let b = hmac_sha256(b"secret", b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn hmac_sha256_differs_with_different_key() {
        let a = hmac_sha256(b"key1", b"data");
        let b = hmac_sha256(b"key2", b"data");
        assert_ne!(a, b);
    }

    #[test]
    fn verify_hmac_accepts_valid() {
        let key = b"my-secret";
        let data = b"{}";
        let sig = hmac_sha256(key, data);
        assert!(verify_hmac(key, data, &sig));
    }

    #[test]
    fn verify_hmac_rejects_invalid() {
        let key = b"my-secret";
        let data = b"{}";
        let bad_sig = [0u8; 32];
        assert!(!verify_hmac(key, data, &bad_sig));
    }

    #[test]
    fn hex_roundtrip() {
        let data = [0xde, 0xad, 0xbe, 0xef];
        let encoded = hex_encode(&data);
        assert_eq!(encoded, "deadbeef");
        assert_eq!(hex_decode(&encoded), Some(data.to_vec()));
    }

    #[test]
    fn webhook_signature_hex() {
        let sig = webhook_signature(b"key", b"body");
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn constant_time_eq_basic() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
