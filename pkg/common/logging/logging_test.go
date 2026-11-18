package logging

import (
	"bytes"
	"context"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestOperationalLogsAreInvocationLocalAndOptIn(t *testing.T) {
	var quiet, verbose bytes.Buffer
	silent := WithLogger(context.Background(), &quiet, false)
	enabled := WithLogger(context.Background(), &verbose, true)
	FromContext(silent).DebugContext(silent, "hidden operation")
	FromContext(enabled).DebugContext(enabled, "pipeline operation", "rule", "diagram")
	require.Empty(t, quiet.String())
	require.Contains(t, verbose.String(), "pipeline operation")
	require.Contains(t, verbose.String(), "rule=diagram")
}
