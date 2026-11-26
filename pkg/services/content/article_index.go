package content

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func articleTimestamp(now time.Time) string {
	if now.IsZero() {
		now = time.Now()
	}
	return now.UTC().Format("2006-01-02T15:04:05Z")
}
func freshArticle(project, path, title, timestamp string) model.Article {
	return model.Article{Title: title, Project: filepath.Base(project), Type: "blog", Path: path, Status: model.ArticleStatusDraft, CreatedAt: timestamp, UpdatedAt: timestamp}
}
func isDirectory(path string) bool { info, err := os.Stat(path); return err == nil && info.IsDir() }
func isRegularFile(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}

func declaredArticlePaths(project string, config *model.MindConfig, articlesDir string) (map[string]string, error) {
	entries := map[string]*string{}
	for id, item := range config.Build.Articles {
		entries[id] = item.ArticleDir
	}
	raw, err := json.Marshal(config.Articles)
	if err != nil {
		return nil, err
	}
	var compat map[string]any
	_ = json.Unmarshal(raw, &compat)
	for id, item := range compat {
		if _, exists := entries[id]; exists {
			continue
		}
		var dir *string
		if object, ok := item.(map[string]any); ok {
			if text, ok := object["article_dir"].(string); ok {
				dir = &text
			}
		}
		entries[id] = dir
	}
	paths := map[string]string{}
	for id, dir := range entries {
		conventional := articlesDir + "/" + id
		if isDirectory(filepath.Join(project, conventional)) {
			paths[id] = conventional
		} else {
			paths[id] = conventional + ".md"
		}
		if dir != nil {
			path, err := store.CanonicalWithin(project, filepath.Join(project, *dir))
			if err != nil {
				return nil, err
			}
			if isRegularFile(filepath.Join(path, id+".md")) {
				paths[id] = *dir + "/" + id + ".md"
			} else if isDirectory(path) {
				paths[id] = *dir
			}
		}
	}
	return paths, nil
}

func ScanArticleDocs(project string, config *model.MindConfig) ([]model.ScannedArticle, error) {
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return nil, err
	}
	scanned := []model.ScannedArticle{}
	path := filepath.Join(project, layout.Articles)
	if _, err := os.Stat(path); err == nil {
		entries, err := os.ReadDir(path)
		if err != nil {
			return nil, err
		}
		for _, entry := range entries {
			name := entry.Name()
			if !utf8.ValidString(name) {
				continue
			}
			if entry.Type().IsRegular() && filepath.Ext(name) == ".md" {
				stem := strings.TrimSuffix(name, ".md")
				dir := layout.Articles
				scanned = append(scanned, model.ScannedArticle{Title: strings.ReplaceAll(stem, "-", " "), Filename: stem, ArticleDir: &dir})
			}
			if entry.IsDir() && !strings.HasPrefix(name, ".") {
				children, err := os.ReadDir(filepath.Join(path, name))
				if err != nil {
					continue
				}
				for _, child := range children {
					if child.Type().IsRegular() && filepath.Ext(child.Name()) == ".md" {
						dir := layout.Articles + "/" + name
						scanned = append(scanned, model.ScannedArticle{Title: strings.ReplaceAll(name, "-", " "), Filename: name, ArticleDir: &dir, ArticlePath: &dir})
						break
					}
				}
			}
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	ids := make([]string, 0, len(config.Build.Articles))
	for id := range config.Build.Articles {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		item := config.Build.Articles[id]
		if item.ArticleDir == nil {
			continue
		}
		dir := *item.ArticleDir
		path, err := store.CanonicalWithin(project, filepath.Join(project, dir))
		if err != nil {
			return nil, err
		}
		if !isDirectory(path) {
			continue
		}
		articlePath := dir
		if isRegularFile(filepath.Join(path, id+".md")) {
			articlePath = dir + "/" + id + ".md"
		}
		scanned = append(scanned, model.ScannedArticle{Title: strings.ReplaceAll(id, "-", " "), Filename: id, ArticleDir: &dir, ArticlePath: &articlePath})
	}
	seen := map[string]bool{}
	unique := []model.ScannedArticle{}
	for _, item := range scanned {
		path := scannedArticlePath(item, layout.Articles)
		if !seen[path] {
			seen[path] = true
			unique = append(unique, item)
		}
	}
	return unique, nil
}

func scannedArticlePath(article model.ScannedArticle, docs string) string {
	if article.ArticlePath != nil {
		return *article.ArticlePath
	}
	if article.ArticleDir != nil {
		docs = *article.ArticleDir
	}
	return docs + "/" + article.Filename + ".md"
}

func ScanTemplateArticles(project string, config *model.MindConfig) ([]model.Article, error) {
	articles := []model.Article{}
	if config.Templates == nil {
		return articles, nil
	}
	names := make([]string, 0, len(*config.Templates))
	for name := range *config.Templates {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		template := (*config.Templates)[name]
		if template.Mode != "generated" {
			continue
		}
		pattern, err := articlePattern(template.Pattern, name)
		if err != nil {
			return nil, err
		}
		err = filepath.WalkDir(project, func(path string, entry os.DirEntry, walkErr error) error {
			if walkErr != nil {
				return nil
			}
			if path != project && strings.HasPrefix(entry.Name(), ".") {
				if entry.IsDir() {
					return filepath.SkipDir
				}
				return nil
			}
			if !entry.Type().IsRegular() || filepath.Ext(entry.Name()) != ".md" {
				return nil
			}
			relative, err := filepath.Rel(project, path)
			if err != nil {
				return nil
			}
			relative = filepath.ToSlash(relative)
			slot, matched := matchArticlePattern(pattern, relative)
			if !matched {
				return nil
			}
			timestamp := articleTimestamp(time.Time{})
			if info, err := entry.Info(); err == nil {
				timestamp = articleTimestamp(info.ModTime())
			}
			article := freshArticle(project, relative, name+"/"+slot, timestamp)
			article.TemplateOrigin = &model.TemplateOrigin{TemplateName: name, SlotValue: slot}
			articles = append(articles, article)
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.SliceStable(articles, func(i, j int) bool { return articles[i].Path < articles[j].Path })
	return articles, nil
}

// DiscoverArticles combines declarations, docs and generated files without writes.
func DiscoverArticles(project string, config *model.MindConfig, now time.Time) (model.Index, []string, error) {
	document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return model.Index{}, nil, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return model.Index{}, nil, err
	}
	existing := map[string]model.Article{}
	if document.Value.Articles != nil {
		for _, article := range *document.Value.Articles {
			existing[strings.TrimRight(article.Path, "/")] = article
		}
	}
	merged := map[string]model.Article{}
	prefixes := []string{}
	timestamp := articleTimestamp(now)
	declared, err := declaredArticlePaths(project, config, layout.Articles)
	if err != nil {
		return model.Index{}, nil, err
	}
	warnings := document.DiagnosticWarnings()
	ids := make([]string, 0, len(declared))
	for id := range declared {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		path := declared[id]
		if _, err := os.Stat(filepath.Join(project, path)); err != nil {
			warnings = append(warnings, "declared article identity '"+path+"' has no content on disk (configured as '"+id+"')")
		}
		article := freshArticle(project, path, strings.ReplaceAll(id, "-", " "), timestamp)
		if old, ok := existing[path]; ok {
			article = old
			article.Path = path
			article.TemplateOrigin = nil
		}
		merged[path] = article
		if !strings.HasSuffix(path, ".md") {
			prefixes = append(prefixes, strings.TrimRight(path, "/")+"/")
		}
	}
	scanned, err := ScanArticleDocs(project, config)
	if err != nil {
		return model.Index{}, nil, err
	}
	for _, item := range scanned {
		path := scannedArticlePath(item, layout.Articles)
		if _, ok := merged[path]; ok {
			continue
		}
		article := freshArticle(project, path, item.Title, timestamp)
		if old, ok := existing[path]; ok {
			article = old
			article.Path = path
		}
		merged[path] = article
	}
	templates, err := ScanTemplateArticles(project, config)
	if err != nil {
		return model.Index{}, nil, err
	}
	for _, article := range templates {
		path := article.Path
		if _, ok := merged[path]; ok {
			continue
		}
		covered := false
		for _, prefix := range prefixes {
			if strings.HasPrefix(path, prefix) {
				covered = true
				break
			}
		}
		if covered {
			continue
		}
		if old, ok := existing[path]; ok {
			origin := article.TemplateOrigin
			article = old
			article.TemplateOrigin = origin
		}
		merged[path] = article
	}
	for path, article := range existing {
		if _, ok := merged[path]; !ok {
			merged[path] = article
		}
	}
	articles := make([]model.Article, 0, len(merged))
	for path, article := range merged {
		if old, ok := existing[path]; ok {
			article.CreatedAt, article.UpdatedAt = old.CreatedAt, old.UpdatedAt
		}
		articles = append(articles, article)
	}
	sort.Slice(articles, func(i, j int) bool { return articles[i].Path < articles[j].Path })
	result := document.Value
	result.Articles = &articles
	return result, warnings, nil
}
