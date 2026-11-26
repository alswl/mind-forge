package build

import (
	"fmt"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/markdown"
)

type Block struct {
	Path    string
	Content string
}

// AssembleBlocks transforms supplied source bytes without reading or writing
// author files. Callers retain ownership of selection, paths and publication.
func AssembleBlocks(blocks []Block, stripTitle bool, banner string) (string, []string, error) {
	var content strings.Builder
	warnings := []string{}
	for i, block := range blocks {
		visibility, err := markdown.BlockVisibility(block.Content)
		if err != nil {
			return "", nil, fmt.Errorf("%s: %w", block.Path, err)
		}
		if visibility == "private" {
			if i == 0 {
				return "", nil, fmt.Errorf("the first block of the article (%s) carries the title and cannot be marked mind-forge-visibility: private", block.Path)
			}
			continue
		}
		body := markdown.StripMindForgeFrontMatter(markdown.StripTyporaFrontMatter(block.Content))
		// Metadata stripping only changes the prefix before the authored body.
		lineOffset := strings.Count(block.Content, "\n") - strings.Count(body, "\n")
		body, notes := stripPrivateCallouts(body)
		for _, note := range notes {
			label := "mf-private callout"
			if note.title != "" {
				label += fmt.Sprintf(" %q", note.title)
			}
			warnings = append(warnings, fmt.Sprintf("%s:%d-%d: %s: quoted continuation after a bare blank line was removed as private", block.Path, note.startLine+lineOffset, note.endLine+lineOffset, label))
		}
		content.WriteString(body)
		if !strings.HasSuffix(content.String(), "\n") {
			content.WriteByte('\n')
		}
	}
	result := content.String()
	if stripTitle {
		result = stripFirstH1(result)
	}
	if banner != "" {
		result = insertBanner(result, banner)
	}
	return result, warnings, nil
}

func stripFirstH1(content string) string {
	_, body, _, ok := markdown.InitialFrontMatter(content)
	start := 0
	if ok {
		start = len(content) - len(body)
	} else if strings.HasPrefix(content, "---\n") || strings.HasPrefix(content, "---\r\n") {
		return content
	}
	for start < len(content) {
		end := len(content)
		if n := strings.IndexByte(content[start:], '\n'); n >= 0 {
			end = start + n + 1
		}
		line := strings.TrimRight(content[start:end], "\r\n")
		if strings.TrimSpace(line) == "" {
			start = end
			continue
		}
		heading := strings.TrimLeft(line, " \t\r\n")
		if heading == "#" || strings.HasPrefix(heading, "# ") || strings.HasPrefix(heading, "#\t") {
			return content[:start] + content[end:]
		}
		break
	}
	return content
}

func insertBanner(content, banner string) string {
	// Preserve the reference's banner split, including the blank after YAML.
	if rest, ok := strings.CutPrefix(content, "---"); ok {
		if end := strings.Index(rest, "\n---"); end >= 0 {
			split := 3 + end + 4
			if split < len(content) && content[split] == '\n' {
				split++
			}
			return content[:split] + "\n" + banner + content[split:]
		}
	}
	return banner + content
}

func rewriteTarget(target, sourceDir, outDir string, warnings *[]string) (string, bool) {
	value, ok := markdown.RebaseRelativeTarget(target, sourceDir, outDir)
	if !ok && markdown.ShouldRewriteTarget(target) {
		*warnings = append(*warnings, fmt.Sprintf("cannot compute a relative path for reference '%s' (source: %s) to output directory '%s'; keeping original reference", target, sourceDir, outDir))
	}
	return value, ok
}

func rewriteRelativePaths(body, sourceDir, outDir string, warnings *[]string) string {
	return markdown.RewriteReferences(body, func(target string) (string, bool) { return rewriteTarget(target, sourceDir, outDir, warnings) })
}
