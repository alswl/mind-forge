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

func privateHeader(line string) bool {
	if !blockquote(line) {
		return false
	}
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	rest := strings.TrimPrefix(trimmed, ">")
	rest = strings.TrimPrefix(rest, " ")
	rest, ok := strings.CutPrefix(rest, "[!")
	if !ok {
		return false
	}
	kind, _, ok := strings.Cut(rest, "]")
	if !ok {
		return false
	}
	kind = strings.ToLower(kind)
	return strings.HasPrefix(kind, "mf-") || strings.HasPrefix(kind, "mind-forge-")
}

// StripPrivateCallouts keeps fenced examples literal. Blank lines followed by
// another quote remain private and produce one warning at the original header.
func StripPrivateCallouts(content string) (string, []PrivateWarning) {
	lines := strings.SplitAfter(content, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	var result bytes.Buffer
	warnings := []PrivateWarning{}
	var fence markdown.FenceTracker
	body := func(line string) string { return strings.TrimRight(line, "\r\n") }
	blank := func(line string) bool { return strings.TrimSpace(body(line)) == "" }
	for i := 0; i < len(lines); {
		line := body(lines[i])
		inside := fence.ProcessLine(line)
		if inside || !privateHeader(line) {
			result.WriteString(lines[i])
			i++
			continue
		}
		start := i + 1
		merged := false
		i++
		for {
			if i < len(lines) && blockquote(body(lines[i])) {
				i++
				continue
			}
			look := i
			for look < len(lines) && blank(lines[look]) {
				look++
			}
			if look > i && look < len(lines) && blockquote(body(lines[look])) {
				merged = true
				i = look
				continue
			}
			break
		}
		if merged {
			warnings = append(warnings, PrivateWarning{StartLine: start})
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
