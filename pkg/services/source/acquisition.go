package source

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type AcquisitionOptions struct {
	MaxBytes     uint64
	Timeout      time.Duration
	MaxRedirects int
}

func DefaultAcquisitionOptions() AcquisitionOptions {
	return AcquisitionOptions{MaxBytes: 64 * 1024 * 1024, Timeout: 30 * time.Second, MaxRedirects: 5}
}

func AcquireHTTP(ctx context.Context, location string, options AcquisitionOptions) ([]byte, error) {
	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.DisableCompression = true
	defer transport.CloseIdleConnections()
	return acquireHTTP(ctx, location, options, transport)
}

func acquireHTTP(ctx context.Context, location string, options AcquisitionOptions, transport http.RoundTripper) ([]byte, error) {
	parsed, err := url.Parse(location)
	if err != nil || parsed.Scheme == "" {
		return nil, output.Usage("registered URL is invalid", "")
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return nil, output.Usage("registered URL must use HTTP or HTTPS", "")
	}
	client := http.Client{Timeout: options.Timeout, Transport: transport, CheckRedirect: func(request *http.Request, via []*http.Request) error {
		if len(via) > options.MaxRedirects {
			return fmt.Errorf("too many redirects")
		}
		return nil
	}}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, parsed.String(), nil)
	if err != nil {
		return nil, httpError("HTTP acquisition failed", err)
	}
	request.Header.Set("User-Agent", "")
	response, err := client.Do(request)
	if err != nil {
		return nil, httpError("HTTP acquisition failed", err)
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, &output.Error{Kind: "io", Message: fmt.Sprintf("HTTP acquisition returned %s", response.Status)}
	}
	if response.ContentLength >= 0 && uint64(response.ContentLength) > options.MaxBytes {
		return nil, &output.Error{Kind: "io", Message: "HTTP response exceeds configured byte limit"}
	}
	limit := options.MaxBytes
	if limit >= uint64(^uint64(0)>>1) {
		limit = uint64(^uint64(0)>>1) - 1
	}
	raw, err := io.ReadAll(io.LimitReader(response.Body, int64(limit+1)))
	if err != nil {
		return nil, httpError("failed to read HTTP response", err)
	}
	if uint64(len(raw)) > options.MaxBytes {
		return nil, &output.Error{Kind: "io", Message: "HTTP response exceeds configured byte limit"}
	}
	return raw, nil
}

func httpError(prefix string, err error) error {
	return &output.Error{Kind: "io", Message: prefix + ": " + err.Error(), Cause: err}
}
