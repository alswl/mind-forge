package content

import (
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/textcase"
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
	return writeTyporaFrontMatter(content, assetsPath, false)
}

func typoraEnabled(config *model.MindConfig) bool {
	return config.Plugins == nil || config.Plugins.Typora == nil || config.Plugins.Typora.Enabled == nil || *config.Plugins.Typora.Enabled
}

const typoraFrontMatterKey = "typora-copy-images-to:"

func writeTyporaFrontMatter(content, assetsPath string, replace bool) string {
	key := typoraFrontMatterKey + " " + assetsPath
	front, body, eol, ok := markdown.InitialFrontMatter(content)
	if !ok {
		return "---\n" + key + "\n---\n\n" + content
	}
	lines := strings.SplitAfter(front, "\n")
	for i, line := range lines {
		if strings.HasPrefix(strings.TrimLeftFunc(line, unicode.IsSpace), typoraFrontMatterKey) {
			if !replace {
				return content
			}
			lines[i] = key + eol
			return "---" + eol + strings.Join(lines, "") + "---" + eol + body
		}
	}
	if front != "" && !strings.HasSuffix(front, "\n") {
		front += eol
	}
	return "---" + eol + front + key + eol + "---" + eol + body
}

func hasTyporaFrontMatter(content string) bool {
	front, _, _, _ := markdown.InitialFrontMatter(content)
	for _, line := range strings.Split(front, "\n") {
		if value, ok := strings.CutPrefix(strings.TrimLeftFunc(line, unicode.IsSpace), typoraFrontMatterKey); ok {
			value = strings.TrimSpace(value)
			return value != "" && value != "''" && value != "\"\"" && !strings.HasPrefix(value, "#")
		}
	}
	return false
}
