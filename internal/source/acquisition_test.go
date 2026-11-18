package source

import (
	"context"
	"io"
	"net/http"
	"strings"
	"testing"
)

type roundTripFunc func(*http.Request) (*http.Response, error)

func (call roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return call(request)
}

func TestAcquisitionUsesBoundedRawBodyAndRejectsStatusOrOverflow(t *testing.T) {
	for _, test := range []struct {
		status int
		length int64
		body   string
		limit  uint64
		want   string
	}{{200, 3, "a\x00b", 3, "a\x00b"}, {200, -1, "abcd", 3, ""}, {200, 4, "abcd", 3, ""}, {404, 0, "", 3, ""}} {
		transport := roundTripFunc(func(request *http.Request) (*http.Response, error) {
			return &http.Response{StatusCode: test.status, Status: http.StatusText(test.status), ContentLength: test.length, Body: io.NopCloser(strings.NewReader(test.body)), Header: http.Header{}, Request: request}, nil
		})
		raw, err := acquireHTTP(context.Background(), "https://example.test/raw", AcquisitionOptions{MaxBytes: test.limit, MaxRedirects: 1}, transport)
		if test.want != "" {
			if err != nil || string(raw) != test.want {
				t.Fatalf("%q %v", raw, err)
			}
		} else if err == nil {
			t.Fatal("unbounded/error response accepted")
		}
	}
}

func TestAcquisitionRedirectLimitUsesInjectedTransport(t *testing.T) {
	calls := 0
	transport := roundTripFunc(func(request *http.Request) (*http.Response, error) {
		calls++
		return &http.Response{StatusCode: 302, Status: "302 Found", ContentLength: 0, Body: io.NopCloser(strings.NewReader("")), Header: http.Header{"Location": []string{"/again"}}, Request: request}, nil
	})
	if _, err := acquireHTTP(context.Background(), "https://example.test/first", AcquisitionOptions{MaxBytes: 8, MaxRedirects: 2}, transport); err == nil || calls != 3 {
		t.Fatalf("redirect bound calls%d err%v", calls, err)
	}
}
