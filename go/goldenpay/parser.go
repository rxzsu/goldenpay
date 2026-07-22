package goldenpay

import (
	"regexp"
	"strconv"
	"strings"

	"golang.org/x/net/html"
)

var userRegex = regexp.MustCompile(`/users/(\d+)/`)

// parseUserFromHome extracts UserInfo from the home page HTML.
func parseUserFromHome(htmlContent, phpsessid string) (*UserInfo, error) {
	doc, err := html.Parse(strings.NewReader(htmlContent))
	if err != nil {
		return nil, wrapError(ErrParse, "parse home page", err)
	}

	var userID int64
	var csrfToken string
	var username string

	// Find data-app-data attribute on <body>
	var bodyAttrs map[string]string
	var findBody func(*html.Node)
	findBody = func(n *html.Node) {
		if n.Type == html.ElementNode && n.Data == "body" {
			for _, a := range n.Attr {
				if a.Key == "data-app-data" {
					bodyAttrs = parseAppData(a.Val)
				}
			}
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			findBody(c)
		}
	}
	findBody(doc)

	if bodyAttrs == nil {
		return nil, newError(ErrUnauthorized, "missing app data")
	}

	userID, _ = strconv.ParseInt(bodyAttrs["userId"], 10, 64)
	csrfToken = bodyAttrs["csrf-token"]

	if userID == 0 || csrfToken == "" {
		return nil, newError(ErrUnauthorized, "incomplete app data")
	}

	// Extract username
	username = extractUsername(doc)
	if strings.TrimSpace(username) == "" {
		return nil, newError(ErrUnauthorized, "empty username")
	}

	return &UserInfo{
		ID:        userID,
		Username:  strings.TrimSpace(username),
		CSRFToken: csrfToken,
		PHPSessID: phpsessid,
	}, nil
}

func extractUsername(doc *html.Node) string {
	var walk func(*html.Node) string
	walk = func(n *html.Node) string {
		if n.Type == html.ElementNode {
			for _, a := range n.Attr {
				if a.Key == "class" && strings.Contains(a.Val, "user-link-name") {
					if c := n.FirstChild; c != nil {
						return strings.TrimSpace(c.Data)
					}
				}
			}
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			if s := walk(c); s != "" {
				return s
			}
		}
		return ""
	}
	return walk(doc)
}

// Simplified app data parser (handles basic key:value JSON)
func parseAppData(s string) map[string]string {
	m := make(map[string]string)
	s = strings.TrimSpace(s)
	s = strings.Trim(s, "{}")
	for _, pair := range strings.Split(s, ",") {
		kv := strings.SplitN(pair, ":", 2)
		if len(kv) == 2 {
			key := strings.Trim(strings.TrimSpace(kv[0]), `"`)
			val := strings.Trim(strings.TrimSpace(kv[1]), `"`)
			m[key] = val
		}
	}
	return m
}
