package build

import (
	"bytes"
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/pkg/common/markdown"
)

type PrivateWarning struct {
	StartLine int
}

func blockquote(line string) bool {
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	return len(line)-len(trimmed) <= 3 && strings.HasPrefix(trimmed, ">")
}

type privateContinuation struct {
	startLine int
	endLine   int
	title     string
}

func privateHeader(line string) (string, bool) {
	if !blockquote(line) {
		return "", false
	}
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	rest := strings.TrimPrefix(trimmed, ">")
	rest = strings.TrimPrefix(rest, " ")
	rest, ok := strings.CutPrefix(rest, "[!")
	if !ok {
		return "", false
	}
	kind, title, ok := strings.Cut(rest, "]")
	if !ok {
		return "", false
	}
	kind = strings.ToLower(kind)
	return strings.TrimSpace(title), strings.HasPrefix(kind, "mf-") || strings.HasPrefix(kind, "mind-forge-")
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
	lines := strings.SplitAfter(content, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	var result bytes.Buffer
	var warnings []privateContinuation
	var fence markdown.FenceTracker
	body := func(line string) string { return strings.TrimRight(line, "\r\n") }
	blank := func(line string) bool { return strings.TrimSpace(body(line)) == "" }
	for i := 0; i < len(lines); {
		line := body(lines[i])
		inside := fence.ProcessLine(line)
		title, isPrivate := privateHeader(line)
		if inside || !isPrivate {
			result.WriteString(lines[i])
			i++
			continue
		}
		start := i + 1
		merged := false
		i++
		for {
			look := i
			for look < len(lines) && blank(lines[look]) {
				look++
			}
			if look == len(lines) || !blockquote(body(lines[look])) {
				break
			}
			if _, nextPrivate := privateHeader(body(lines[look])); nextPrivate {
				break
			}
			merged = merged || look > i
			i = look + 1
		}
		if merged {
			warnings = append(warnings, privateContinuation{startLine: start, endLine: i, title: title})
		}
		for bytes.HasSuffix(result.Bytes(), []byte("\n\n\n")) {
			result.Truncate(result.Len() - 1)
		}
		for i < len(lines) && blank(lines[i]) {
			i++
		}
		if result.Len() > 0 && !bytes.HasSuffix(result.Bytes(), []byte("\n\n")) && i < len(lines) {
			result.WriteByte('\n')
		}
	}
	return result.String(), warnings
}
