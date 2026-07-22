package goldenpay

import (
	"io"
	"net/http"
	"strings"
	"time"
)

// GoldenPay is the reusable HTTP client.
type GoldenPay struct {
	http   *http.Client
	config *GoldenPayConfig
	urls   *Urls
}

// GoldenPaySession is an authenticated session.
type GoldenPaySession struct {
	client *GoldenPay
	user   *UserInfo
}

func New(config *GoldenPayConfig) *GoldenPay {
	transport := &http.Transport{}
	if config.Proxy != "" {
		// proxy would be set here
	}
	return &GoldenPay{
		http: &http.Client{
			Transport: transport,
			Timeout:   30 * time.Second,
		},
		config: config,
		urls:   NewUrls(config.BaseURL),
	}
}

func (c *GoldenPay) Connect() (*GoldenPaySession, error) {
	req, err := http.NewRequest("GET", c.urls.Home(), nil)
	if err != nil {
		return nil, wrapError(ErrHTTP, "connect request", err)
	}
	req.Header.Set("User-Agent", c.config.UserAgent)
	req.Header.Set("Cookie", "golden_key="+c.config.GoldenKey+"; cookie_prefs=1")

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, wrapError(ErrHTTP, "connect", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusForbidden {
		return nil, newError(ErrUnauthorized, "invalid golden key")
	}

	body, _ := io.ReadAll(resp.Body)

	// Extract PHPSESSID from Set-Cookie
	var phpsessid string
	for _, c := range resp.Cookies() {
		if c.Name == "PHPSESSID" {
			phpsessid = c.Value
			break
		}
	}

	user, err := parseUserFromHome(string(body), phpsessid)
	if err != nil {
		return nil, err
	}

	return &GoldenPaySession{client: c, user: user}, nil
}

func (s *GoldenPaySession) User() *UserInfo       { return s.user }
func (s *GoldenPaySession) Config() *GoldenPayConfig { return s.client.config }

func (s *GoldenPaySession) cookieHeader() string {
	cookie := "golden_key=" + s.client.config.GoldenKey + "; cookie_prefs=1"
	if s.user.PHPSessID != "" {
		cookie += "; PHPSESSID=" + s.user.PHPSessID
	}
	return cookie
}

func (s *GoldenPaySession) getHTML(url string) (string, error) {
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("User-Agent", s.client.config.UserAgent)
	req.Header.Set("Cookie", s.cookieHeader())
	req.Header.Set("Accept", "*/*")

	resp, err := s.client.http.Do(req)
	if err != nil {
		return "", wrapError(ErrHTTP, "get", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusForbidden {
		return "", newError(ErrUnauthorized, "session expired")
	}

	body, _ := io.ReadAll(resp.Body)
	return string(body), nil
}

func (s *GoldenPaySession) postForm(url, payload, referer, accept string) (string, error) {
	req, _ := http.NewRequest("POST", url, strings.NewReader(payload))
	req.Header.Set("User-Agent", s.client.config.UserAgent)
	req.Header.Set("Cookie", s.cookieHeader())
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
	req.Header.Set("Accept", accept)
	req.Header.Set("X-Requested-With", "XMLHttpRequest")
	if referer != "" {
		req.Header.Set("Referer", referer)
	}

	resp, err := s.client.http.Do(req)
	if err != nil {
		return "", wrapError(ErrHTTP, "post", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	return string(body), nil
}

func (s *GoldenPaySession) SetGoldenKey(key string) {
	s.client.config.GoldenKey = key
}

func (s *GoldenPaySession) CheckConnection() bool {
	_, err := s.getHTML(s.client.urls.Home())
	return err == nil
}
