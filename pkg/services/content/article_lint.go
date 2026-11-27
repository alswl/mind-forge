package content

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func LintArticles(project string, fix bool) ([]model.LintIssue, error) {
	return LintArticlesByStatus(project, fix, nil)
}

func LintArticlesByStatus(project string, fix bool, status *string) ([]model.LintIssue, error) {
	return LintArticlesRequest(context.Background(), project, project, fix, status)
}

func LintArticlesRequest(ctx context.Context, root, project string, fix bool, status *string) ([]model.LintIssue, error) {
	return lintArticles(ctx, root, project, fix, status, false)
}

func LintFinalArticles(ctx context.Context, project string, status *string) ([]model.LintIssue, error) {
	return lintArticles(ctx, project, project, false, status, true)
}

func lintArticles(ctx context.Context, root, project string, fix bool, status *string, final bool) ([]model.LintIssue, error) {
	if err := validateArticleStatus(status); err != nil {
		return nil, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return nil, err
	}
	var finalRules []finalLintRule
	if final {
		finalRules, err = compileFinalLintRules(config.ArticleLint)
		if err != nil {
			return nil, err
		}
	}
	docs := filepath.Join(project, layout.Articles)
	entries, err := os.ReadDir(docs)
	if os.IsNotExist(err) {
		return []model.LintIssue{}, nil
	}
	if err != nil {
		return nil, err
	}
	statuses := map[string]string{}
	if status != nil || fix {
		index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
		if err != nil {
			return nil, err
		}
		if index.Value.Articles != nil {
			for _, article := range *index.Value.Articles {
				statuses[filepath.Clean(article.Path)] = article.Status
			}
		}
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
	lintBlock := lintTypora
	if final {
		lintBlock = func(relative, path string, body []byte) error {
			if err := ctx.Err(); err != nil {
				return err
			}
			if !utf8.Valid(body) {
				return fmt.Errorf("stream did not contain valid UTF-8: %s", path)
			}
			issues = appendFinalLintIssues(issues, relative, string(body), finalRules)
			return nil
		}
	}
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		name := entry.Name()
		relative := filepath.Join(layout.Articles, name)
		current, registered := statuses[relative]
		if status != nil {
			if !registered {
				current = model.ArticleStatusDraft
			}
			if current != *status {
				continue
			}
		}
		if entry.IsDir() && (final || checkTypora) && !strings.HasPrefix(name, ".") {
			if err := lintArticleBlocks(project, relative, lintBlock); err != nil {
				return nil, err
			}
			continue
		}
		if !utf8.ValidString(name) || filepath.Ext(name) != ".md" ||
			final && (strings.HasPrefix(name, ".") || !entry.Type().IsRegular()) {
			continue
		}
		stem := strings.TrimSuffix(name, ".md")
		relative = filepath.ToSlash(relative)
		path, err := CheckedPath(project, relative)
		if err != nil {
			return nil, err
		}
		body, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if final {
			if err := lintBlock(relative, path, body); err != nil {
				return nil, err
			}
			continue
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
					if registered {
						newPath := filepath.ToSlash(filepath.Join(layout.Articles, expected+".md"))
						mutation, err := planLintArticleRename(ctx, root, project, relative, newPath)
						if err != nil {
							return nil, err
						}
						if err := ApplyMutation(ctx, root, mutation); err != nil {
							return nil, err
						}
					} else if err := os.Rename(path, target); err != nil {
						return nil, err
					}
				}
			}
		}
	}
	return issues, nil
}

func lintArticleBlocks(project, relative string, lint func(string, string, []byte) error) error {
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
