package content

import (
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

// PreviewRefreshedArticle derives the article cache without publishing any index
// or role projection. It reuses the same scanners as the explicit index command.
func PreviewRefreshedArticle(project, selector string, config *model.MindConfig) (model.Article, error) {
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return model.Article{}, err
	}
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), true)
	if err != nil {
		return model.Article{}, err
	}
	scanned, err := ScanArticleDocs(project, config)
	if err != nil {
		return model.Article{}, err
	}
	diff := ArticleDiff(index.Value, scanned, layout.Articles)
	removed := map[string]bool{}
	for _, a := range diff.Removed {
		removed[a.Path] = true
	}
	articles := []model.Article{}
	covered := map[string]bool{}
	if index.Value.Articles != nil {
		for _, a := range *index.Value.Articles {
			if !removed[a.Path] {
				articles = append(articles, a)
				covered[a.Path] = true
			}
		}
	}
	timestamp := articleTimestamp(time.Now())
	for _, a := range diff.Added {
		path := scannedArticlePath(a, layout.Articles)
		articles = append(articles, freshArticle(project, path, a.Title, timestamp))
		covered[path] = true
	}
	declared, err := declaredArticlePaths(project, config, layout.Articles)
	if err != nil {
		return model.Article{}, err
	}
	for name, path := range declared {
		if !covered[path] {
			articles = append(articles, freshArticle(project, path, strings.ReplaceAll(name, "-", " "), timestamp))
			covered[path] = true
		}
	}
	generated, err := ScanTemplateArticles(project, config)
	if err != nil {
		return model.Article{}, err
	}
	for _, a := range generated {
		if covered[a.Path] {
			continue
		}
		skip := false
		for _, item := range config.Build.Articles {
			if item.ArticleDir != nil && strings.HasPrefix(a.Path, strings.TrimRight(*item.ArticleDir, "/")+"/") {
				skip = true
				break
			}
		}
		if !skip {
			articles = append(articles, a)
			covered[a.Path] = true
		}
	}
	sort.SliceStable(articles, func(i, j int) bool { return articles[i].Path < articles[j].Path })
	return ResolveIndexedEntries(articles, selector)
}
