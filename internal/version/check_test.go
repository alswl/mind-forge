package version

import (
	"context"
	"errors"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
)

type releaseTransport func(*http.Request) (*http.Response, error)

func (transport releaseTransport) RoundTrip(request *http.Request) (*http.Response, error) {
	return transport(request)
}

func releaseResponse(status int, body string) releaseTransport {
	return func(*http.Request) (*http.Response, error) {
		return &http.Response{StatusCode: status, Body: io.NopCloser(strings.NewReader(body))}, nil
	}
}

const stableRelease = `{"tag_name":"v0.6.3+build.01","draft":false,"prerelease":false,"html_url":"https://example.invalid"}`

func TestCheckComparesVersions(t *testing.T) {
	for _, tc := range []struct {
		current, status string
	}{
		{"0.0.1", "update_available"},
		{"0.6.3-dev+abc123", "update_available"},
		{"0.6.3-rc.1", "update_available"},
		{"0.6.3-01rc", "update_available"},
		{"0.6.3", "up_to_date"},
		{"0.6.3+build.00", "up_to_date"},
		{"0.6.10", "ahead"},
		{"0.10.0", "ahead"},
		{"0.7.0-dev+abc123", "ahead"},
		{"999999999999999999999.0.0", "ahead"},
	} {
		t.Run(tc.current, func(t *testing.T) {
			result, err := check(t.Context(), tc.current, releaseResponse(http.StatusOK, stableRelease))
			require.NoError(t, err)
			require.Equal(t, tc.current, result.CurrentVersion)
			require.Equal(t, "0.6.3+build.01", result.LatestVersion)
			require.Equal(t, tc.status, result.Status)
			if tc.status == "update_available" {
				require.Equal(t, "https://github.com/alswl/mind-forge/blob/master/docs/guide/install.md", result.UpgradeURL)
			} else {
				require.Empty(t, result.UpgradeURL)
			}
		})
	}
}

func TestCheckRejectsMalformedLocalVersions(t *testing.T) {
	for _, current := range []string{"0.06.3", "0.6", "0.6.3-rc.01", "0.6.3-", "0.6.3+", "0.6.3+foo..bar", "0.6.3\n", "v0.6.3"} {
		t.Run(current, func(t *testing.T) {
			transport := releaseTransport(func(*http.Request) (*http.Response, error) {
				t.Fatal("invalid local version sent an HTTP request")
				return nil, nil
			})
			_, err := check(t.Context(), current, transport)
			require.ErrorContains(t, err, "invalid local version")
		})
	}
}

func TestCheckRequestsOfficialEndpoint(t *testing.T) {
	transport := releaseTransport(func(request *http.Request) (*http.Response, error) {
		require.Equal(t, http.MethodGet, request.Method)
		require.Equal(t, "https://api.github.com/repos/alswl/mind-forge/releases/latest", request.URL.String())
		require.Equal(t, "application/vnd.github+json", request.Header.Get("Accept"))
		require.Nil(t, request.Body)
		deadline, ok := request.Context().Deadline()
		require.True(t, ok)
		require.Positive(t, time.Until(deadline))
		require.LessOrEqual(t, time.Until(deadline), 5*time.Second)
		return releaseResponse(http.StatusOK, stableRelease)(request)
	})
	_, err := check(t.Context(), "0.6.3", transport)
	require.NoError(t, err)
}

func TestCheckRejectsHTTPFailureAndRedirect(t *testing.T) {
	for _, status := range []int{http.StatusFound, http.StatusForbidden, http.StatusNotFound, http.StatusTooManyRequests, http.StatusServiceUnavailable} {
		t.Run(http.StatusText(status), func(t *testing.T) {
			calls := 0
			transport := releaseTransport(func(request *http.Request) (*http.Response, error) {
				calls++
				response, err := releaseResponse(status, stableRelease)(request)
				response.Header = http.Header{"Location": {"https://example.invalid"}}
				return response, err
			})
			_, err := check(t.Context(), "0.6.3", transport)
			require.ErrorContains(t, err, "HTTP ")
			require.Equal(t, 1, calls)
		})
	}
}

func TestCheckRejectsInvalidReleaseResponses(t *testing.T) {
	for _, tc := range []struct {
		name, body, message string
	}{
		{"malformed", "{", "invalid GitHub release JSON"},
		{"trailing JSON", stableRelease + "{}", "invalid GitHub release JSON"},
		{"wrong field type", `{"tag_name":42}`, "invalid GitHub release JSON"},
		{"oversized", stableRelease + strings.Repeat(" ", maxReleaseBody), "response exceeds"},
		{"missing flags", `{"tag_name":"v0.6.3"}`, "published stable SemVer release"},
		{"null flags", `{"tag_name":"v0.6.3","draft":null,"prerelease":false}`, "published stable SemVer release"},
		{"draft", `{"tag_name":"v0.6.3","draft":true,"prerelease":false}`, "published stable SemVer release"},
		{"prerelease", `{"tag_name":"v0.6.3","draft":false,"prerelease":true}`, "published stable SemVer release"},
		{"prerelease tag", `{"tag_name":"v0.6.3-rc.1","draft":false,"prerelease":false}`, "published stable SemVer release"},
		{"invalid tag", `{"tag_name":"v0.06.3","draft":false,"prerelease":false}`, "published stable SemVer release"},
		{"untagged", `{"tag_name":"0.6.3","draft":false,"prerelease":false}`, "published stable SemVer release"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			_, err := check(t.Context(), "0.6.3", releaseResponse(http.StatusOK, tc.body))
			require.ErrorContains(t, err, tc.message)
		})
	}
}

func TestCheckNetworkError(t *testing.T) {
	networkError := errors.New("connection refused")
	transport := releaseTransport(func(*http.Request) (*http.Response, error) {
		return nil, networkError
	})
	_, err := check(t.Context(), "0.6.3", transport)
	require.ErrorContains(t, err, "requesting the latest release from GitHub")
	require.ErrorIs(t, err, networkError)
}

func TestCheckFollowsParentCancellationAndDeadline(t *testing.T) {
	t.Run("cancel", func(t *testing.T) {
		ctx, cancel := context.WithCancel(t.Context())
		defer cancel()
		transport := releaseTransport(func(request *http.Request) (*http.Response, error) {
			cancel()
			require.ErrorIs(t, request.Context().Err(), context.Canceled)
			return nil, request.Context().Err()
		})
		_, err := check(ctx, "0.6.3", transport)
		require.ErrorIs(t, err, context.Canceled)
	})
	t.Run("deadline", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(t.Context(), 10*time.Millisecond)
		defer cancel()
		transport := releaseTransport(func(request *http.Request) (*http.Response, error) {
			deadline, ok := request.Context().Deadline()
			require.True(t, ok)
			parentDeadline, _ := ctx.Deadline()
			require.Equal(t, parentDeadline, deadline)
			<-request.Context().Done()
			return nil, request.Context().Err()
		})
		_, err := check(ctx, "0.6.3", transport)
		require.ErrorIs(t, err, context.DeadlineExceeded)
	})
}
