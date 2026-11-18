package content

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func ArticleOutputStem(path string) string {
	path = strings.TrimSuffix(path, ".md")
	if rest, ok := strings.CutPrefix(path, "docs/"); ok {
		path = rest
	} else {
		path = strings.TrimPrefix(path, "outputs/")
	}
	return strings.TrimRight(path, "/")
}

// ResolveArticleSelector uses exact identity/title matches for write operations.
func ResolveArticleSelector(project, selector string) (string, error) {
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return "", err
	}
	return resolveArticleEntries(index.Value, selector)
}

func resolveArticleEntries(index model.Index, selector string) (string, error) {
	candidates := map[string]bool{}
	if index.Articles != nil {
		stem := ArticleOutputStem(selector)
		for _, article := range *index.Articles {
			if article.Title == selector || ArticleOutputStem(article.Path) == stem {
				candidates[article.Path] = true
			}
		}
	}
	paths := make([]string, 0, len(candidates))
	for path := range candidates {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	switch len(paths) {
	case 0:
		hint := "use `mf article list --project <project>` to see available articles"
		return "", &output.Error{Kind: "not_found", Message: fmt.Sprintf("article '%s' not found", selector), Hint: &hint}
	case 1:
		return paths[0], nil
	default:
		quoted := make([]string, len(paths))
		for i, path := range paths {
			quoted[i] = "'" + path + "'"
		}
		return "", output.Usage(fmt.Sprintf("selector '%s' is ambiguous: matches %s", selector, strings.Join(quoted, ", ")), "pass the full article path to disambiguate")
	}
}
