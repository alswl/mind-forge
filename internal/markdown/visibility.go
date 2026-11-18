package markdown

import (
	"fmt"
	"strings"
	"unicode"
)

// InitialFrontMatter returns only a complete opening YAML block.
func InitialFrontMatter(content string) (front, body, eol string, ok bool) {
	opening := 4
	eol = "\n"
	if strings.HasPrefix(content, "---\r\n") {
		opening, eol = 5, "\r\n"
	} else if !strings.HasPrefix(content, "---\n") {
		return "", content, "", false
	}
	remaining := content[opening:]
	for offset := 0; offset < len(remaining); {
		next := len(remaining)
		if end := strings.IndexByte(remaining[offset:], '\n'); end >= 0 {
			next = offset + end + 1
		}
		if strings.TrimRight(remaining[offset:next], "\r\n") == "---" {
			return remaining[:offset], remaining[next:], eol, true
		}
		offset = next
	}
	return "", content, "", false
}

func BlockVisibility(content string) (string, error) {
	front, _, _, ok := InitialFrontMatter(content)
	if !ok {
		return "public", nil
	}
	for _, line := range strings.Split(front, "\n") {
		line = strings.TrimRight(strings.TrimLeftFunc(line, unicode.IsSpace), "\r\n")
		if rest, ok := strings.CutPrefix(line, "mind-forge-visibility:"); ok {
			value := strings.Trim(strings.TrimSpace(rest), "\"'")
			if value == "public" || value == "private" {
				return value, nil
			}
			return "", fmt.Errorf("invalid mind-forge-visibility value '%s': expected 'public' or 'private'", value)
		}
	}
	return "public", nil
}
