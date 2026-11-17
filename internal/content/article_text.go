package content

import (
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/internal/markdown"
	"github.com/alswl/mind-forge/internal/textcase"
)

func ArticleFilename(raw string) string {
	var result strings.Builder
	dash := false
	for _, r := range textcase.Lower(raw) {
		if textcase.IsAlphanumeric(r) {
			result.WriteRune(r)
			dash = false
		} else if result.Len() > 0 && !dash {
			result.WriteByte('-')
			dash = true
		}
	}
	value := strings.TrimRight(result.String(), "-")
	if value == "" {
		return "untitled"
	}
	return value
}

func InjectTyporaFrontMatter(content, assetsPath string) string {
	key := "typora-copy-images-to: " + assetsPath
	front, body, eol, ok := markdown.InitialFrontMatter(content)
	if !ok {
		return "---\n" + key + "\n---\n\n" + content
	}
	for _, line := range strings.Split(front, "\n") {
		if strings.HasPrefix(strings.TrimLeftFunc(line, unicode.IsSpace), "typora-copy-images-to:") {
			return content
		}
	}
	if front != "" && !strings.HasSuffix(front, "\n") {
		front += eol
	}
	return "---" + eol + front + key + eol + "---" + eol + body
}
