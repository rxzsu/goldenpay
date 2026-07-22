package goldenpay

import "fmt"

// GoldenPayError represents SDK errors.
type GoldenPayError struct {
	Kind    ErrorKind
	Message string
	Err     error
}

type ErrorKind int

const (
	ErrMissingGoldenKey ErrorKind = iota
	ErrUnauthorized
	ErrHTTP
	ErrParse
	ErrIO
	ErrDelivery
	ErrState
)

func (e *GoldenPayError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("%s: %v", e.Message, e.Err)
	}
	return e.Message
}

func (e *GoldenPayError) Unwrap() error { return e.Err }

func newError(kind ErrorKind, msg string) *GoldenPayError {
	return &GoldenPayError{Kind: kind, Message: msg}
}

func wrapError(kind ErrorKind, msg string, err error) *GoldenPayError {
	return &GoldenPayError{Kind: kind, Message: msg, Err: err}
}
