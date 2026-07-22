package goldenpay

import (
	"crypto/hmac"
	"encoding/json"
	"encoding/hex"
	"io"
	"log"
	"net"
	"net/http"
)

// WebhookConfig for the notification server.
type WebhookConfig struct {
	BindAddr    string // e.g. "127.0.0.1:9090"
	Endpoint    string // e.g. "/webhook"
	Secret      string // HMAC secret (empty = disabled)
	MaxBodySize int64
}

func DefaultWebhookConfig() *WebhookConfig {
	return &WebhookConfig{
		BindAddr:    "127.0.0.1:9090",
		Endpoint:    "/webhook",
		MaxBodySize: 1 << 20, // 1 MB
	}
}

// WebhookPayload carries the parsed request.
type WebhookPayload struct {
	SourceIP string
	Body     json.RawMessage
	Headers  map[string]string
}

// WebhookHandler processes incoming webhook events.
type WebhookHandler interface {
	HandleWebhook(payload WebhookPayload) error
}

// WebhookServer receives POST notifications.
type WebhookServer struct {
	config  *WebhookConfig
	handler WebhookHandler
}

func NewWebhookServer(config *WebhookConfig, handler WebhookHandler) *WebhookServer {
	return &WebhookServer{config: config, handler: handler}
}

func (s *WebhookServer) Run() error {
	mux := http.NewServeMux()
	mux.HandleFunc(s.config.Endpoint, s.handle)

	addr := s.config.BindAddr
	if addr == "" {
		addr = "127.0.0.1:9090"
	}

	listener, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}
	log.Printf("webhook server listening on %s%s", addr, s.config.Endpoint)
	return http.Serve(listener, mux)
}

func (s *WebhookServer) handle(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Only POST allowed", http.StatusMethodNotAllowed)
		return
	}

	body, err := io.ReadAll(io.LimitReader(r.Body, s.config.MaxBodySize))
	if err != nil {
		http.Error(w, "Cannot read body", http.StatusBadRequest)
		return
	}

	// HMAC verification
	if s.config.Secret != "" {
		sigHeader := r.Header.Get("X-Signature-256")
		sig, err := hex.DecodeString(sigHeader)
		if err != nil || !hmac.Equal(HMACSHA256([]byte(s.config.Secret), body), sig) {
			http.Error(w, "Invalid signature", http.StatusUnauthorized)
			return
		}
	}

	headers := make(map[string]string)
	for k, v := range r.Header {
		if len(v) > 0 {
			headers[k] = v[0]
		}
	}

	payload := WebhookPayload{
		SourceIP: r.RemoteAddr,
		Body:     body,
		Headers:  headers,
	}

	if err := s.handler.HandleWebhook(payload); err != nil {
		log.Printf("webhook handler error: %v", err)
	}

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("OK"))
}
