package output

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestProgressRequiresBothTTYTextNonquietAndUsableTerm(t *testing.T) {
	for _, out := range []bool{false, true} {
		for _, err := range []bool{false, true} {
			for _, format := range []string{"text", "json"} {
				for _, quiet := range []bool{false, true} {
					for _, term := range []string{"xterm", "dumb"} {
						enabled := ProgressEnabled(ProgressInput{StdoutIsTTY: out, StderrIsTTY: err, Format: format, Quiet: quiet, Term: term})
						require.Equal(t, out && err && format == "text" && !quiet && term != "dumb", enabled)
					}
				}
			}
		}
	}
}

func TestProgressIsPlainSingleLineIncludingNoColorPolicy(t *testing.T) {
	// The presenter never emits ANSI, regardless of NO_COLOR or style settings.
	line := ProgressText(task.Stage{Operation: "source", Name: "reading response", Label: "synthetic"})
	require.Equal(t, "source: reading response (synthetic)\n", line)
	line = ProgressText(task.Stage{Operation: "build\x1b", Name: "step\r\n", Label: "safe\a"})
	require.NotContains(t, line, "\x1b")
	require.NotContains(t, line, "\r")
	require.NotContains(t, line, "\a")
	require.Equal(t, 1, strings.Count(line, "\n"))
}
