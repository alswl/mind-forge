package content

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type ArticlePromptView struct {
	Path          string   `json:"path"`
	Mode          *string  `json:"mode,omitempty"`
	BindingStatus string   `json:"binding_status"`
	UpdatedAt     string   `json:"updated_at"`
	Conflicts     []string `json:"conflicts,omitempty"`
}
type ArticleThinkingView struct {
	Path      string `json:"path"`
	UpdatedAt string `json:"updated_at"`
}
type ArticleListItem struct {
	Article  model.Article
	Data     map[string]any
	Prompt   *ArticlePromptView
	Thinking *ArticleThinkingView
	Mtime    uint64
	Project  string
}

func articleBindings(index model.Index) (map[string]*ArticlePromptView, map[string]*ArticleThinkingView) {
	groups := map[string][]promptBinding{}
	for _, binding := range resolvePromptBindings(index) {
		if binding.Prompt.Article != "" {
			groups[binding.Prompt.Article] = append(groups[binding.Prompt.Article], binding)
		}
	}
	prompts := map[string]*ArticlePromptView{}
	for path, group := range groups {
		sort.Slice(group, func(i, j int) bool { return group[i].Prompt.Path < group[j].Prompt.Path })
		primary := group[0].Prompt
		view := &ArticlePromptView{Path: primary.Path, Mode: primary.Mode, BindingStatus: group[0].Status, UpdatedAt: primary.UpdatedAt}
		if len(group) > 1 {
			for _, prompt := range group {
				view.Conflicts = append(view.Conflicts, prompt.Prompt.Path)
			}
		}
		prompts[path] = view
	}
	thinking := map[string]*ArticleThinkingView{}
	if index.Thinking != nil {
		for _, item := range *index.Thinking {
			base := filepath.Base(item.Path)
			key := strings.TrimSuffix(base, filepath.Ext(base))
			thinking[key] = &ArticleThinkingView{Path: item.Path, UpdatedAt: item.UpdatedAt}
		}
	}
	return prompts, thinking
}

func EffectiveArticleDir(config *model.MindConfig, article model.Article) string {
	layout, _ := app.ResolveEffectiveLayout(config)
	key := strings.TrimSuffix(article.Path, ".md")
	short := strings.TrimPrefix(key, layout.Articles+"/")
	if item, ok := config.Build.Articles[short]; ok && item.ArticleDir != nil {
		return *item.ArticleDir
	}
	ids := make([]string, 0, len(config.Build.Articles))
	for id := range config.Build.Articles {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		item := config.Build.Articles[id]
		if item.ArticleDir != nil && *item.ArticleDir == article.Path {
			return *item.ArticleDir
		}
	}
	return layout.Articles + "/" + short
}

func ArticleContentKind(project, path string) string {
	full := filepath.Join(project, path)
	if isDirectory(full) {
		return "blocked"
	}
	if isRegularFile(full) {
		return "single_file"
	}
	return "missing"
}

func ArticleFileMtime(project, path string) uint64 {
	visited := map[string]bool{}
	var scan func(string) uint64
	scan = func(path string) uint64 {
		info, err := os.Stat(path)
		if err != nil {
			return 0
		}
		if !info.IsDir() {
			seconds := info.ModTime().Unix()
			if seconds < 0 {
				return 0
			}
			return uint64(seconds)
		}
		canonical, err := filepath.EvalSymlinks(path)
		if err != nil || visited[canonical] {
			return 0
		}
		visited[canonical] = true
		children, err := os.ReadDir(path)
		if err != nil {
			return 0
		}
		newest := uint64(0)
		for _, entry := range children {
			newest = max(newest, scan(filepath.Join(path, entry.Name())))
		}
		return newest
	}
	return scan(filepath.Join(project, path))
}

func ListArticleItems(project string) ([]ArticleListItem, []string, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, nil, err
	}
	index, warnings, err := DiscoverArticles(project, config, time.Time{})
	if err != nil {
		return nil, nil, err
	}
	document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return nil, nil, err
	}
	prompts, thinking := articleBindings(document.Value)
	raw, err := json.Marshal(config.Articles)
	if err != nil {
		return nil, nil, err
	}
	var compat map[string]any
	_ = json.Unmarshal(raw, &compat)
	result := []ArticleListItem{}
	for _, article := range *index.Articles {
		raw, err := json.Marshal(article)
		if err != nil {
			return nil, nil, err
		}
		data := map[string]any{}
		if err := json.Unmarshal(raw, &data); err != nil {
			return nil, nil, err
		}
		origin := "docs"
		if article.TemplateOrigin != nil {
			origin = "generated"
		} else {
			short := ArticleOutputStem(article.Path)
			_, typed := config.Build.Articles[short]
			_, legacy := compat[short]
			if typed || legacy {
				origin = "declared"
			}
		}
		mtime := ArticleFileMtime(project, article.Path)
		prompt, thought := prompts[article.Path], thinking[ArticleOutputStem(article.Path)]
		data["article_dir"], data["id"] = EffectiveArticleDir(config, article), strings.TrimRight(strings.TrimSuffix(article.Path, ".md"), "/")
		data["origin"], data["article_present"], data["content_kind"] = origin, ArticleContentKind(project, article.Path) != "missing", ArticleContentKind(project, article.Path)
		data["identity"], data["path"], data["mtime"], data["prompt"], data["thinking"] = article.Path, article.Path, mtime, prompt, thought
		result = append(result, ArticleListItem{Article: article, Data: data, Prompt: prompt, Thinking: thought, Mtime: mtime, Project: project})
	}
	sort.SliceStable(result, func(i, j int) bool {
		if result[i].Mtime == result[j].Mtime {
			return result[i].Article.Path < result[j].Article.Path
		}
		return result[i].Mtime > result[j].Mtime
	})
	return result, warnings, nil
}

func ShowArticle(project, selector string) (ArticleListItem, []string, error) {
	if err := store.RejectTraversal(selector, "project"); err != nil {
		return ArticleListItem{}, nil, err
	}
	if _, err := store.CanonicalWithin(project, filepath.Join(project, selector)); err != nil {
		return ArticleListItem{}, nil, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return ArticleListItem{}, nil, err
	}
	discovered, warnings, err := DiscoverArticles(project, config, time.Time{})
	if err != nil {
		return ArticleListItem{}, nil, err
	}
	articles := *discovered.Articles
	var selected *model.Article
	for i := range articles {
		if articles[i].Path == selector {
			selected = &articles[i]
			break
		}
	}
	if selected == nil {
		for i := range articles {
			a := &articles[i]
			if textcase.ASCIIEqualFold(a.Title, selector) || textcase.ASCIIEqualFold(ArticleOutputStem(a.Path), selector) || strings.Contains(a.Path, selector) {
				selected = a
				break
			}
		}
	}
	if selected == nil {
		return ArticleListItem{}, warnings, output.Usage("article '"+selector+"' not found", "use `mf article list` to see available articles")
	}
	document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return ArticleListItem{}, nil, err
	}
	prompts, _ := articleBindings(document.Value)
	var thought *ArticleThinkingView
	if document.Value.Thinking != nil {
		for _, item := range *document.Value.Thinking {
			base := filepath.Base(item.Path)
			if strings.TrimSuffix(base, filepath.Ext(base)) == ArticleOutputStem(selected.Path) {
				thought = &ArticleThinkingView{Path: item.Path, UpdatedAt: item.UpdatedAt}
				break
			}
		}
	}
	return ArticleListItem{Article: *selected, Prompt: prompts[selected.Path], Thinking: thought, Project: project}, warnings, nil
}

func ArticleProjectRoots(root string) ([]string, error) {
	manifest, err := loadManifest(root)
	if err != nil {
		return nil, err
	}
	result := []string{}
	seen := map[string]bool{}
	add := func(path string) {
		if !seen[path] {
			seen[path] = true
			result = append(result, path)
		}
	}
	for _, entry := range manifest.Projects {
		path := entry.Path
		if !filepath.IsAbs(path) {
			path = filepath.Join(root, strings.TrimPrefix(path, "./"))
		}
		add(path)
	}
	container := filepath.Join(root, strings.Trim(manifest.ProjectsDir, "/"))
	if entries, err := os.ReadDir(container); err == nil {
		for _, entry := range entries {
			path := filepath.Join(container, entry.Name())
			if isDirectory(path) && isRegularFile(filepath.Join(path, "mind.yaml")) {
				add(path)
			}
		}
	}
	return result, nil
}
