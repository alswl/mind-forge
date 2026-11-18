package main

import (
	"fmt"
	"strings"
)

func ExamplesText(examples []HelpExample) string {
	if len(examples) == 0 {
		return ""
	}
	var text strings.Builder
	text.WriteString("Examples (use a disposable directory for setup):\n")
	for _, example := range examples {
		text.WriteString("  Prerequisites:\n")
		for _, step := range example.Prerequisites {
			fmt.Fprintf(&text, "    %s\n", step)
		}
		fmt.Fprintf(&text, "  Command:\n    %s\n  Effect: %s\n", example.Command, example.Effect)
	}
	return text.String()
}
