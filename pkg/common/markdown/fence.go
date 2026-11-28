package markdown

import (
	"strings"
	"unicode"
)

// FenceTracker includes opening and closing marker lines in the fenced region.
// It preserves the reference Markdown scanner's marker rules for all consumers.
type FenceTracker struct {
	char   byte
	length int
}

func (tracker *FenceTracker) ProcessLine(line string) bool {
	return tracker.processLine(line, false)
}

func (tracker *FenceTracker) processLine(line string, markdownFences bool) bool {
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	if len(line)-len(trimmed) <= 3 && len(trimmed) >= 3 {
		char := trimmed[0]
		if char == '`' || char == '~' {
			length := 0
			for length < len(trimmed) && trimmed[length] == char {
				length++
			}
			rest := trimmed[length:]
			blankRest := strings.Trim(rest, " \t\r\n\x0b\x0c") == ""
			valid := length >= 3 && (char == '~' && (markdownFences || blankRest) ||
				char == '`' && !strings.Contains(rest, "`"))
			if valid {
				if tracker.char == 0 {
					tracker.char, tracker.length = char, length
					return true
				}
				if tracker.char == char && length >= tracker.length && (!markdownFences || blankRest) {
					tracker.char, tracker.length = 0, 0
					return true
				}
			}
		}
	}
	return tracker.char != 0
}
