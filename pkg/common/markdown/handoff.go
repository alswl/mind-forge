package markdown

import (
	"fmt"
	"slices"
	"strings"
)

type HandoffFields struct {
	Decision  string `json:"decision"`
	Rework    string `json:"rework"`
	OpenItems string `json:"open_items"`
	NextStep  string `json:"next_step"`
}

type HandoffCallout struct {
	StartLine   int
	EndLine     int
	Fields      HandoffFields
	Diagnostics []string
}

// HandoffCallouts finds structured handoffs in private Markdown ranges.
func HandoffCallouts(source string) []HandoffCallout {
	_, body, _, _ := InitialFrontMatter(source)
	offset := strings.Count(source[:len(source)-len(body)], "\n")
	lines := strings.Split(body, "\n")
	var records []HandoffCallout
	for _, callout := range PrivateCallouts(lines) {
		header, _ := privateQuote(strings.TrimSuffix(lines[callout.StartLine-1], "\r"))
		marker, _, _ := strings.Cut(header, "]")
		if callout.Title != "Handoff" || !strings.EqualFold(marker, "[!mf-private") {
			continue
		}
		start := offset + callout.StartLine
		fields, diagnostics := parseHandoffFields(lines[callout.StartLine:callout.EndLine], start)
		records = append(records, HandoffCallout{start, offset + callout.EndLine, fields, diagnostics})
	}
	return records
}

func parseHandoffFields(lines []string, headerLine int) (HandoffFields, []string) {
	names := []string{"Decision", "Rework", "Open items", "Next step"}
	var values [4][]string
	var diagnostics []string
	current := -1
	for i, line := range lines {
		lineNumber := headerLine + i + 1
		quoted, ok := privateQuote(strings.TrimSuffix(line, "\r"))
		if ok && quoted == "" {
			continue
		}
		if continuation, ok := strings.CutPrefix(quoted, "  "); ok && current >= 0 {
			values[current] = append(values[current], continuation)
			continue
		}
		name, value, ok := strings.Cut(quoted, ":")
		current = slices.Index(names, name)
		switch {
		case !ok || current < 0:
			diagnostics = append(diagnostics, fmt.Sprintf("line %d: expected a handoff field or a quoted continuation", lineNumber))
			current = -1
		case values[current] != nil:
			diagnostics = append(diagnostics, fmt.Sprintf("line %d: duplicate %s field", lineNumber, name))
			current = -1
		default:
			values[current] = []string{strings.TrimSpace(value)}
		}
	}
	var fields [4]string
	for i, name := range names {
		if values[i] == nil {
			diagnostics = append(diagnostics, "missing "+name+" field")
		}
		fields[i] = strings.Trim(strings.Join(values[i], "\n"), "\n")
	}
	return HandoffFields{fields[0], fields[1], fields[2], fields[3]}, diagnostics
}
