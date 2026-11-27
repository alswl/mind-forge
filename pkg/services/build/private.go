package build

import (
	"bytes"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/markdown"
)

type PrivateWarning struct {
	StartLine int
}

type privateContinuation struct {
	startLine int
	endLine   int
	title     string
}

// StripPrivateCallouts keeps fenced examples literal. Blank lines followed by
// a quoted continuation remain private and produce one warning at its header.
func StripPrivateCallouts(content string) (string, []PrivateWarning) {
	body, continuations := stripPrivateCallouts(content)
	warnings := make([]PrivateWarning, 0, len(continuations))
	for _, note := range continuations {
		warnings = append(warnings, PrivateWarning{StartLine: note.startLine})
	}
	return body, warnings
}

func stripPrivateCallouts(content string) (string, []privateContinuation) {
	_, body, _, _ := markdown.InitialFrontMatter(content)
	prefix := content[:len(content)-len(body)]
	offset := strings.Count(prefix, "\n")
	lines := strings.SplitAfter(body, "\n")
	if lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	var result bytes.Buffer
	result.WriteString(prefix)
	var warnings []privateContinuation
	first := 0
	for _, callout := range markdown.PrivateCallouts(lines) {
		for _, line := range lines[first : callout.StartLine-1] {
			result.WriteString(line)
		}
		if callout.Continued {
			warnings = append(warnings, privateContinuation{startLine: offset + callout.StartLine, endLine: offset + callout.EndLine, title: callout.Title})
		}
		for bytes.HasSuffix(result.Bytes(), []byte("\n\n\n")) {
			result.Truncate(result.Len() - 1)
		}
		first = callout.EndLine
		for first < len(lines) && strings.TrimSpace(lines[first]) == "" {
			first++
		}
		if result.Len() > 0 && !bytes.HasSuffix(result.Bytes(), []byte("\n\n")) && first < len(lines) {
			result.WriteByte('\n')
		}
	}
	for _, line := range lines[first:] {
		result.WriteString(line)
	}
	return result.String(), warnings
}
