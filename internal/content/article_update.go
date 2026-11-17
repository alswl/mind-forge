package content

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type ArticleUpdateOptions struct {
	Title, Status *string
	DryRun        bool
	Now           time.Time
}
type ArticleUpdateReport struct {
	Article model.Article  `json:"article"`
	DryRun  bool           `json:"dry_run"`
	Changes map[string]any `json:"changes"`
}

func UpdateArticle(project, selector string, options ArticleUpdateOptions) (ArticleUpdateReport, error) {
	report := ArticleUpdateReport{DryRun: options.DryRun, Changes: map[string]any{}}
	if options.Status != nil && *options.Status != "draft" && *options.Status != "published" {
		return report, output.Usage(fmt.Sprintf("invalid status '%s'", *options.Status), "use --status draft or --status published")
	}
	if options.Title == nil && options.Status == nil {
		return report, output.Usage("nothing to update: use --status and/or --title", "pass --status draft|published or --title \"New Title\"")
	}
	indexPath := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(indexPath, false)
	if err != nil {
		return report, err
	}
	updated := document.Value
	position := -1
	articles := []model.Article{}
	if updated.Articles != nil {
		articles = append(articles, (*updated.Articles)...)
		for i, article := range articles {
			if article.Title == selector || article.Path == selector {
				position = i
				break
			}
		}
	}
	if position < 0 {
		hint := "use `mf article list --project <project>` to see available articles"
		return report, &output.Error{Kind: "not_found", Message: fmt.Sprintf("article '%s' not found", selector), Hint: &hint}
	}
	article := &articles[position]
	now := options.Now
	if now.IsZero() {
		now = time.Now()
	}
	timestamp := now.UTC().Format("2006-01-02T15:04:05Z")
	if options.Status != nil {
		report.Changes["status"] = map[string]any{"from": article.Status, "to": *options.Status}
		if !options.DryRun {
			article.Status = *options.Status
			article.UpdatedAt = timestamp
		}
	}
	if options.Title != nil {
		title := strings.TrimSpace(*options.Title)
		if title == "" {
			return report, output.Usage("title must not be empty", "pass a non-empty title")
		}
		report.Changes["title"] = map[string]any{"from": article.Title, "to": title}
		if !options.DryRun {
			article.Title = title
			article.UpdatedAt = timestamp
			path, err := CheckedPath(project, "prompts/"+ArticleOutputStem(article.Path)+".md")
			if err != nil {
				return report, err
			}
			if raw, err := os.ReadFile(path); err == nil && utf8.Valid(raw) {
				body := updatePromptTitle(string(raw), title)
				_ = (store.AtomicWriter{}).Write(path, []byte(body), 0o644)
			}
		}
	}
	updated.Articles = &articles
	report.Article = *article
	if !options.DryRun {
		err = document.Write(indexPath, updated, false)
	}
	return report, err
}

func updatePromptTitle(content, title string) string {
	lines := strings.Split(strings.ReplaceAll(content, "\r\n", "\n"), "\n")
	trailing := strings.HasSuffix(content, "\n")
	if trailing {
		lines = lines[:len(lines)-1]
	}
	inFront := false
	for i, line := range lines {
		if strings.TrimSpace(line) == "---" {
			if inFront {
				break
			}
			inFront = true
			continue
		}
		if inFront && strings.HasPrefix(line, "title:") {
			lines[i] = "title: " + title
		}
	}
	result := strings.Join(lines, "\n")
	if trailing {
		result += "\n"
	}
	return result
}
