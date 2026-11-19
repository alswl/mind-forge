package source

import (
	"context"
	"io"
	"net/http"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestHTTPStagesUseInjectedTransportWithoutLeakingLocation(t *testing.T) {
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	transport := roundTripFunc(func(request *http.Request) (*http.Response, error) {
		require.Equal(t, []task.Stage{{Operation: "source", Name: "requesting HTTP source"}}, stages)
		return &http.Response{StatusCode: 200, ContentLength: 9, Body: io.NopCloser(strings.NewReader("synthetic")), Header: http.Header{}, Request: request}, nil
	})
	raw, err := acquireHTTP(ctx, "https://example.invalid/private-query?token=synthetic", AcquisitionOptions{MaxBytes: 32}, transport)
	require.NoError(t, err)
	require.Equal(t, "synthetic", string(raw))
	require.Equal(t, []task.Stage{{Operation: "source", Name: "requesting HTTP source"}, {Operation: "source", Name: "reading HTTP response"}}, stages)
}
