package content

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
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
	checkTypora := typoraEnabled(config)
	lintTypora := func(relative, path string, body []byte) error {
		if !checkTypora || hasTyporaFrontMatter(string(body)) {
			return nil
		}
		assets, err := typoraAssetsPath(project, layout.Assets, filepath.Dir(path))
		if err != nil {
			return err
		}
		issues = append(issues, model.LintIssue{
			Severity: "warning", Kind: "missing_typora_front_matter", Path: relative,
			Message: "add " + typoraFrontMatterKey + " " + assets + " to the file's opening YAML front matter",
		})
		return nil
	}
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() && checkTypora && !strings.HasPrefix(name, ".") {
			if err := lintArticleBlockTypora(project, filepath.Join(layout.Articles, name), lintTypora); err != nil {
				return nil, err
			}
			continue
		}
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
		if err := lintTypora(relative, path, body); err != nil {
			return nil, err
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

func lintArticleBlockTypora(project, relative string, lint func(string, string, []byte) error) error {
	directory, err := CheckedPath(project, relative)
	if err != nil {
		return err
	}
	entries, err := os.ReadDir(directory)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if !entry.Type().IsRegular() || filepath.Ext(entry.Name()) != ".md" || strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		block := filepath.Join(relative, entry.Name())
		path, err := CheckedPath(project, block)
		if err != nil {
			return err
		}
		body, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if err := lint(filepath.ToSlash(block), path, body); err != nil {
			return err
		}
	}
	return nil
}
