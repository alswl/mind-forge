package markdown

import (
	"path/filepath"
	"strings"
	"unicode"
)

func StripTyporaFrontMatter(content string) string {
	return stripFrontMatterKeys(content, func(line string) bool {
		return strings.HasPrefix(strings.TrimLeftFunc(line, unicode.IsSpace), "typora-copy-images-to:")
	})
}

func StripMindForgeFrontMatter(content string) string {
	return stripFrontMatterKeys(content, func(line string) bool {
		return strings.HasPrefix(strings.TrimLeftFunc(line, unicode.IsSpace), "mind-forge-")
	})
}

func stripFrontMatterKeys(content string, remove func(string) bool) string {
	front, body, eol, ok := InitialFrontMatter(content)
	if !ok {
		return content
	}
	var kept strings.Builder
	removed := false
	for _, line := range strings.SplitAfter(front, "\n") {
		if remove(line) {
			removed = true
		} else {
			kept.WriteString(line)
		}
	}
	if !removed {
		return content
	}
	if strings.TrimSpace(kept.String()) == "" {
		return strings.TrimPrefix(body, eol)
	}
	text := kept.String()
	if text != "" && !strings.HasSuffix(text, "\n") {
		text += eol
	}
	return "---" + eol + text + "---" + eol + body
}

func ShouldRewriteTarget(target string) bool {
	if target == "" {
		return false
	}
	for _, prefix := range []string{"/", "http://", "https://", "mailto:", "data:", "#"} {
		if strings.HasPrefix(target, prefix) {
			return false
		}
	}
	return true
}

func RebaseRelativeTarget(target, sourceDir, baseDir string) (string, bool) {
	if !ShouldRewriteTarget(target) {
		return "", false
	}
	resolved := filepath.Clean(filepath.Join(sourceDir, target))
	base := filepath.Clean(baseDir)
	if filepath.IsAbs(base) != filepath.IsAbs(resolved) {
		return "", false
	}
	if filepath.IsAbs(base) {
		parts := func(path string) []string {
			return strings.FieldsFunc(filepath.ToSlash(path), func(r rune) bool { return r == '/' })
		}
		left, right := parts(base), parts(resolved)
		if len(left) == 0 || len(right) == 0 || left[0] != right[0] {
			return "", false
		}
	}
	relative, err := filepath.Rel(base, resolved)
	return filepath.ToSlash(relative), err == nil
}

// RewriteReferences preserves the reference scanner's line and fence semantics.
func RewriteReferences(content string, mapTarget func(string) (string, bool)) string {
	var result strings.Builder
	var fence FenceTracker
	lines := strings.Split(strings.ReplaceAll(content, "\r\n", "\n"), "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	for _, line := range lines {
		if fence.ProcessLine(line) {
			result.WriteString(line)
		} else {
			result.WriteString(rewriteLineReferences(line, mapTarget))
		}
		result.WriteByte('\n')
	}
	return result.String()
}

func rewriteLineReferences(line string, mapTarget func(string) (string, bool)) string {
	trimmed := strings.TrimLeftFunc(line, unicode.IsSpace)
	if strings.HasPrefix(trimmed, "[") && strings.Contains(trimmed, "]: ") {
		if end := strings.IndexByte(trimmed, ']'); end >= 0 {
			if after, ok := strings.CutPrefix(trimmed[end+1:], ": "); ok {
				fields := strings.Fields(after)
				target := ""
				if len(fields) > 0 {
					target = fields[0]
				}
				if next, ok := mapTarget(target); ok {
					return line[:len(line)-len(trimmed)] + trimmed[:end+1] + ": " + next + after[len(target):]
				}
			}
		}
		return line
	}
	return rewriteHTMLImages(rewriteMarkdownBrackets(line, mapTarget), mapTarget)
}

func rewriteMarkdownBrackets(line string, mapTarget func(string) (string, bool)) string {
	var result strings.Builder
	last := 0
	seenOpen := false
	for i := 0; i+1 < len(line); i++ {
		if line[i] == '[' {
			seenOpen = true
		}
		if line[i] != ']' || !seenOpen || line[i+1] != '(' {
			continue
		}
		open := i + 1
		end := strings.IndexByte(line[open+1:], ')')
		if end < 0 {
			continue
		}
		end += open + 1
		if next, ok := mapTarget(line[open+1 : end]); ok {
			result.WriteString(line[last : open+1])
			result.WriteString(next)
			result.WriteByte(')')
			i, last = end, end+1
		}
	}
	if last == 0 {
		return line
	}
	result.WriteString(line[last:])
	return result.String()
}

func rewriteHTMLImages(line string, mapTarget func(string) (string, bool)) string {
	var result strings.Builder
	for {
		start := strings.Index(line, "<img")
		if start < 0 {
			result.WriteString(line)
			break
		}
		result.WriteString(line[:start])
		tag := line[start:]
		end := strings.IndexByte(tag, '>')
		if end < 0 {
			result.WriteString(tag)
			break
		}
		tag = tag[:end+1]
		if src := strings.Index(tag, "src="); src >= 0 && src+4 < len(tag) {
			quote := tag[src+4]
			if quote == '\'' || quote == '"' {
				if close := strings.IndexByte(tag[src+5:], quote); close >= 0 {
					close += src + 5
					if target, ok := mapTarget(tag[src+5 : close]); ok {
						tag = tag[:src+5] + target + tag[close:]
					}
				}
			}
		}
		result.WriteString(tag)
		line = line[start+end+1:]
	}
	return result.String()
}
