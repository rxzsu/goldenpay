package goldenpay

import "time"

// GoldenPayConfig holds runtime configuration.
type GoldenPayConfig struct {
	GoldenKey            string        `json:"golden_key"`
	BaseURL              string        `json:"base_url"`
	UserAgent            string        `json:"user_agent"`
	PollInterval         time.Duration `json:"poll_interval"`
	MaxRetries           int           `json:"max_retries"`
	RetryBaseDelay       time.Duration `json:"retry_base_delay"`
	MaxConcurrentRequests int          `json:"max_concurrent_requests"`
	Proxy                string        `json:"proxy,omitempty"`
	StatePath            string        `json:"state_path,omitempty"`
}

func DefaultConfig() *GoldenPayConfig {
	return &GoldenPayConfig{
		BaseURL:              "https://funpay.com",
		UserAgent:            "goldenpay/1.1.0",
		PollInterval:         2 * time.Second,
		MaxRetries:           3,
		RetryBaseDelay:       300 * time.Millisecond,
		MaxConcurrentRequests: 0,
	}
}

func NewConfig(goldenKey string) *GoldenPayConfig {
	c := DefaultConfig()
	c.GoldenKey = goldenKey
	return c
}
