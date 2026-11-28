package markdown

import (
	"strings"
	"unicode"
)

// PrivateCallout identifies an inclusive, one-based range in the original lines.
type PrivateCallout struct {
	StartLine, EndLine int
	Title              string
	Continued          bool
}

// PrivateCallouts finds private ranges without changing the source or its line numbers.
func PrivateCallouts(lines []string) []PrivateCallout {
	var result []PrivateCallout
	var fence FenceTracker
	for i := 0; i < len(lines); {
		line := strings.TrimRight(lines[i], "\r\n")
		inside := fence.processLine(line, true)
		quoted, _ := privateQuote(line)
		title, private := privateHeader(quoted)
		i++
		if inside || !private {
			continue
		}
		callout := PrivateCallout{StartLine: i, Title: title}
		var quotedFence FenceTracker
		for {
			next := i
			for next < len(lines) && strings.TrimSpace(lines[next]) == "" {
				next++
			}
			if next == len(lines) {
				break
			}
			quoted, ok := privateQuote(strings.TrimRight(lines[next], "\r\n"))
			if !ok {
				break
			}
			if !quotedFence.processLine(quoted, true) {
				if _, nextPrivate := privateHeader(quoted); nextPrivate {
					break
				}
			}
			callout.Continued = callout.Continued || next > i
			i = next + 1
		}
		callout.EndLine = i
		result = append(result, callout)
	}
	return result
}

func privateQuote(line string) (string, bool) {
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	content, quoted := strings.CutPrefix(trimmed, ">")
	if !quoted || len(line)-len(trimmed) > 3 {
		return "", false
	}
	return strings.TrimPrefix(content, " "), true
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
