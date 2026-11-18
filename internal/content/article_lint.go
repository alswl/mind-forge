package content

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/markdown"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/textcase"
)

func LintArticles(project string, fix bool) ([]model.LintIssue, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return nil, err
	}
	docs := filepath.Join(project, layout.Articles)
	entries, err := os.ReadDir(docs)
	if os.IsNotExist(err) {
		return []model.LintIssue{}, nil
	}
	if err != nil {
		return nil, err
	}
	issues := []model.LintIssue{}
	for _, entry := range entries {
		name := entry.Name()
		if !utf8.ValidString(name) || filepath.Ext(name) != ".md" {
			continue
		}
		stem := strings.TrimSuffix(name, ".md")
		relative := layout.Articles + "/" + name
		path, err := CheckedPath(project, relative)
		if err != nil {
			return nil, err
		}
		body, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if !utf8.Valid(body) {
			return nil, fmt.Errorf("stream did not contain valid UTF-8: %s", path)
		}
		add := func(severity, kind, message string, fixable bool) {
			issues = append(issues, model.LintIssue{Severity: severity, Kind: kind, Message: message, Path: relative, Fixable: fixable})
		}
		if strings.TrimSpace(string(body)) == "" {
			add("error", "empty_file", "article file is empty", false)
		}
		visibility, err := markdown.BlockVisibility(string(body))
		if err != nil {
			add("error", "mind_forge_visibility_invalid", err.Error(), false)
		} else if visibility == "private" {
			add("error", "mind_forge_private_title_block", "the article's only block carries the H1 title and cannot be marked mind-forge-visibility: private", false)
		}
		valid := true
		for _, char := range stem {
			if char != '-' && (char < 'a' || char > 'z') && (char < '0' || char > '9') {
				valid = false
				break
			}
		}
		if !valid {
			expected := strings.Map(func(char rune) rune {
				if char == '-' || char >= 'a' && char <= 'z' || char >= '0' && char <= '9' {
					return char
				}
				return '-'
			}, textcase.Lower(stem))
			add("warning", "filename_convention", fmt.Sprintf("filename '%s' should be lowercase with hyphens (suggest: '%s.md')", stem, expected), true)
			if fix {
				target := filepath.Join(docs, expected+".md")
				if _, err := os.Stat(target); os.IsNotExist(err) {
					if err := os.Rename(path, target); err != nil {
						return nil, err
					}
				}
			}
		}
	}
	return issues, nil
}
