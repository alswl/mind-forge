package content

import (
	"fmt"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type ArticleBlock struct{ Filename, Body string }

func BuiltinArticleTemplate(name string) (string, string, bool) {
	opening := "# {title}\n\n> Created: {created_at}\n"
	switch name {
	case "blank":
		return opening, name, true
	case "arch":
		return opening + "\n## Context\n\n## Decision\n\n## Consequence\n\n## Alternatives Considered\n", name, true
	case "prd":
		return opening + "\n## Background\n\n## Goals\n\n## Non-Goals\n\n## Requirements\n", name, true
	case "blog":
		return opening + "\n## Summary\n\n## Content\n", name, true
	default:
		return "", "", false
	}
}

func SplitArticleTemplate(body string) ([]ArticleBlock, error) {
	normalized := strings.ReplaceAll(body, "\r\n", "\n")
	lines := strings.Split(normalized, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	blocks := []ArticleBlock{}
	current := []string{}
	name := "01-opening.md"
	headings := map[string]string{}
	var fence markdown.FenceTracker
	for _, line := range lines {
		inside := fence.ProcessLine(line)
		if heading, ok := strings.CutPrefix(line, "## "); ok && !inside {
			blocks = append(blocks, ArticleBlock{name, strings.Join(current, "\n")})
			slug := ArticleFilename(strings.TrimSpace(heading))
			if previous, exists := headings[slug]; exists {
				hint := "rename one of the headings to produce a distinct filename"
				return nil, &output.Error{Kind: "duplicate_block_slug", Message: fmt.Sprintf("duplicate block slug '%s' from headings '%s' and '%s'", slug, previous, heading), Hint: &hint}
			}
			headings[slug] = heading
			name = fmt.Sprintf("%02d-%s.md", len(blocks)+1, slug)
			current = []string{line}
		} else {
			current = append(current, line)
		}
	}
	blocks = append(blocks, ArticleBlock{name, strings.Join(current, "\n")})
	return blocks, nil
}
