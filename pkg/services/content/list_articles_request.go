package content

import (
	"sort"
	"strings"

	app "github.com/alswl/mind-forge/pkg/config"
)

type ArticleListResult struct {
	Items   []ArticleListItem
	Project string
	All     bool
}

// ListArticlesRequest emits diagnostics before loading the next project so a
// failed diagnostic writer retains the command's original short-circuit order.
func ListArticlesRequest(root, cwd string, selected *string, diagnostic func([]string) error) (ArticleListResult, error) {
	return listArticlesRequest(root, cwd, selected, diagnostic, ArticleProjectRoots, ListArticleItems)
}

func listArticlesRequest(root, cwd string, selected *string, diagnostic func([]string) error, projectRoots func(string) ([]string, error), loadItems func(string) ([]ArticleListItem, []string, error)) (ArticleListResult, error) {
	project, err := app.ResolveProject(root, selected, cwd, false)
	all := selected == nil && err != nil && strings.HasPrefix(err.Error(), "could not detect current project;")
	if err != nil && !all {
		return ArticleListResult{}, err
	}
	result := ArticleListResult{Items: []ArticleListItem{}, Project: project, All: all}
	if all {
		roots, err := projectRoots(root)
		if err != nil {
			return ArticleListResult{}, err
		}
		for _, project := range roots {
			items, warnings, loadErr := loadItems(project)
			if diagnostic != nil {
				if err := diagnostic(warnings); err != nil {
					return ArticleListResult{}, err
				}
			}
			if loadErr == nil {
				result.Items = append(result.Items, items...)
			}
		}
		sort.SliceStable(result.Items, func(i, j int) bool {
			if result.Items[i].Mtime == result.Items[j].Mtime {
				return result.Items[i].Article.Path < result.Items[j].Article.Path
			}
			return result.Items[i].Mtime > result.Items[j].Mtime
		})
	} else {
		items, warnings, err := loadItems(project)
		if err != nil {
			return ArticleListResult{}, err
		}
		if diagnostic != nil {
			if err := diagnostic(warnings); err != nil {
				return ArticleListResult{}, err
			}
		}
		result.Items = items
	}
	return result, nil
}
