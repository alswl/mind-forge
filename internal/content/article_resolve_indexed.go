package content

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

// ResolveIndexedArticle follows the build/publish identity precedence. Display
// titles are deliberately absent: they need not be unique or stable.
func ResolveIndexedArticle(project, selector string) (model.Article, error) {
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), true)
	if err != nil {
		return model.Article{}, err
	}
	var articles []model.Article
	if index.Value.Articles != nil {
		articles = *index.Value.Articles
	}
	return ResolveIndexedEntries(articles, selector)
}

// ResolveIndexedEntries also serves read-only previews of a refreshed projection.
func ResolveIndexedEntries(articles []model.Article, selector string) (model.Article, error) {
	notFound := func(message, hint string) (model.Article, error) {
		return model.Article{}, &output.Error{Kind: "not_found", Message: message, Hint: &hint}
	}
	if len(articles) == 0 {
		return notFound(fmt.Sprintf("no articles indexed; article '%s' not found", selector), "run `mf article index` to index articles")
	}
	predicates := []func(model.Article) bool{
		func(a model.Article) bool {
			return strings.TrimRight(strings.TrimSuffix(a.Path, ".md"), "/") == selector
		},
		func(a model.Article) bool { return a.Path == selector },
		func(a model.Article) bool { return ArticleOutputStem(a.Path) == selector },
		func(a model.Article) bool {
			return a.TemplateOrigin != nil && a.TemplateOrigin.TemplateName+"/"+a.TemplateOrigin.SlotValue == selector
		},
	}
	for _, match := range predicates {
		for _, article := range articles {
			if match(article) {
				return article, nil
			}
		}
	}
	var matches []model.Article
	for _, article := range articles {
		_, rest, ok := strings.Cut(article.Path, "/")
		if article.Path == "docs/"+selector || article.Path == "docs/"+selector+".md" || ok && strings.TrimSuffix(rest, ".md") == selector {
			matches = append(matches, article)
		}
	}
	if len(matches) == 1 {
		return matches[0], nil
	}
	if len(matches) > 1 {
		return model.Article{}, output.Usage(fmt.Sprintf("ambiguous article '%s'; found %d matches", selector, len(matches)), "use the canonical article key from `mf article list`")
	}
	return notFound(fmt.Sprintf("article '%s' not found", selector), "run `mf article list` to see available articles")
}
