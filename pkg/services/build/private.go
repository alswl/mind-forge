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

func quoteContent(line string) (string, bool) {
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	content, quoted := strings.CutPrefix(trimmed, ">")
	if !quoted || len(line)-len(trimmed) > 3 {
		return "", false
	}
	return strings.TrimPrefix(content, " "), true
}

type privateContinuation struct {
	startLine int
	endLine   int
	title     string
}

func privateHeader(quoted string) (string, bool) {
	rest, ok := strings.CutPrefix(quoted, "[!")
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
		quoted, _ := quoteContent(line)
		title, isPrivate := privateHeader(quoted)
		if inside || !isPrivate {
			result.WriteString(lines[i])
			i++
			continue
		}
		start := i + 1
		merged := false
		var quotedFence markdown.FenceTracker
		i++
		for {
			look := i
			for look < len(lines) && blank(lines[look]) {
				look++
			}
			if look == len(lines) {
				break
			}
			quoted, ok := quoteContent(body(lines[look]))
			if !ok {
				break
			}
			if !quotedFence.ProcessLine(quoted) {
				if _, nextPrivate := privateHeader(quoted); nextPrivate {
					break
				}
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
