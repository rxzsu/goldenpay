package goldenpay

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
)

// HMACSHA256 computes HMAC-SHA256 of data with the given key.
func HMACSHA256(key, data []byte) []byte {
	mac := hmac.New(sha256.New, key)
	mac.Write(data)
	return mac.Sum(nil)
}

// WebhookSignature returns the hex-encoded HMAC-SHA256 for webhook headers.
func WebhookSignature(secret string, body []byte) string {
	return hex.EncodeToString(HMACSHA256([]byte(secret), body))
}

// VerifyHMAC checks if signature is a valid HMAC-SHA256 of data under key.
func VerifyHMAC(key, data, signature []byte) bool {
	return hmac.Equal(HMACSHA256(key, data), signature)
}

// SecureString masks its value in string output (always returns "***").
type SecureString string

func (s SecureString) String() string  { return "***" }
func (s SecureString) GoString() string { return "***" }
func (s SecureString) Value() string    { return string(s) }
