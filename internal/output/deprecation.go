package output

import (
	"fmt"
	"io"
	"strings"
)

func WarnDeprecation(writer io.Writer, subject, replacement string, noColor bool) error {
	message := fmt.Sprintf("[deprecated] %s is deprecated, use %s instead\n", subject, replacement)
	if noColor {
		message = stripANSI(message)
	}
	_, err := io.WriteString(writer, message)
	return err
}

func stripANSI(text string) string {
	var out strings.Builder
	escape := false
	for _, char := range text {
		if escape {
			if char == 'm' {
				escape = false
			}
			continue
		}
		if char == '\x1b' {
			escape = true
			continue
		}
		out.WriteRune(char)
	}
	return out.String()
}
