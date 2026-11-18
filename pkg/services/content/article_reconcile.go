package content

import (
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func ArticleDiff(index model.Index, scanned []model.ScannedArticle, docs string) model.ArticleDiff {
	diff := model.ArticleDiff{Added: model.JSONArray[model.ScannedArticle]{}, Removed: model.JSONArray[model.Article]{}}
	paths, names := map[string]bool{}, map[string]bool{}
	for _, entry := range scanned {
		paths[scannedArticlePath(entry, docs)] = true
		names[entry.Filename] = true
	}
	indexed := map[string]bool{}
	if index.Articles != nil {
		for _, article := range *index.Articles {
			trimmed := strings.TrimRight(article.Path, "/")
			indexed[trimmed] = true
			if paths[trimmed] {
				continue
			}
			rest, inDocs := strings.CutPrefix(article.Path, docs+"/")
			stem, md := strings.CutSuffix(rest, ".md")
			if !inDocs || !md || !names[stem] {
				diff.Removed = append(diff.Removed, article)
			}
		}
	}
	for _, entry := range scanned {
		if !indexed[scannedArticlePath(entry, docs)] {
			diff.Added = append(diff.Added, entry)
		}
	}
	return diff
}

func roleIndexDetails[T any](added, removed model.JSONArray[T], kept uint64) map[string]any {
	return map[string]any{"added": added, "removed": removed, "kept_count": kept, "scanned_count": uint64(len(added)) + kept}
}

func IndexArticles(project string, dryRun bool, now time.Time) (output.VerbResult, []string, error) {
	result := output.VerbResult{Verb: output.Index, Kind: "article", DryRun: dryRun}
	config, err := ConfigFor(project)
	if err != nil {
		return result, nil, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return result, nil, err
	}
	scanned, err := ScanArticleDocs(project, config)
	if err != nil {
		return result, nil, err
	}
	path := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(path, false)
	if err != nil {
		return result, nil, err
	}
	diff := ArticleDiff(document.Value, scanned, layout.Articles)
	templatesCount := 0
	if config.Templates != nil {
		for _, template := range *config.Templates {
			if template.Mode == "generated" {
				templatesCount++
			}
		}
	}
	kept := 0
	if document.Value.Articles != nil {
		kept = len(*document.Value.Articles) - len(diff.Removed)
	}
	warnings := []string{}
	details := map[string]any{"kept_count": kept, "scanned_count": len(scanned), "templates_scanned": templatesCount}
	if dryRun {
		details["added"], details["removed"] = []model.ScannedArticle(diff.Added), []model.Article(diff.Removed)
	} else {
		updated := document.Value
		removed := map[string]bool{}
		for _, article := range diff.Removed {
			removed[article.Path] = true
		}
		articles := []model.Article{}
		covered := map[string]bool{}
		if updated.Articles != nil {
			for _, article := range *updated.Articles {
				if !removed[article.Path] {
					articles = append(articles, article)
					covered[article.Path] = true
				}
			}
		}
		timestamp := articleTimestamp(now)
		for _, item := range diff.Added {
			article := freshArticle(project, scannedArticlePath(item, layout.Articles), item.Title, timestamp)
			articles = append(articles, article)
			covered[article.Path] = true
		}
		declared, err := declaredArticlePaths(project, config, layout.Articles)
		if err != nil {
			return result, nil, err
		}
		ids := make([]string, 0, len(declared))
		for id := range declared {
			ids = append(ids, id)
		}
		sort.Strings(ids)
		for _, id := range ids {
			target := declared[id]
			if ArticleContentKind(project, target) == "missing" {
				warnings = append(warnings, "declared article identity '"+target+"' has no content on disk (configured as '"+id+"')")
			}
			if !covered[target] {
				articles = append(articles, freshArticle(project, target, strings.ReplaceAll(id, "-", " "), timestamp))
				covered[target] = true
			}
		}
		generated, err := ScanTemplateArticles(project, config)
		if err != nil {
			return result, nil, err
		}
		prefixes := []string{}
		for _, item := range config.Build.Articles {
			if item.ArticleDir != nil {
				prefixes = append(prefixes, strings.TrimRight(*item.ArticleDir, "/")+"/")
			}
		}
		for _, article := range generated {
			if covered[article.Path] {
				continue
			}
			skip := false
			for _, prefix := range prefixes {
				if strings.HasPrefix(article.Path, prefix) {
					skip = true
					break
				}
			}
			if skip {
				continue
			}
			articles = append(articles, article)
			covered[article.Path] = true
		}
		sort.SliceStable(articles, func(i, j int) bool { return articles[i].Path < articles[j].Path })
		updated.Articles = &articles
		if err := document.Write(path, updated, false); err != nil {
			return result, warnings, err
		}
		details["kept_count"] = len(articles)
		addedJSON, removedJSON := []map[string]any{}, []map[string]any{}
		for _, item := range diff.Added {
			addedJSON = append(addedJSON, map[string]any{"identity": item.ArticlePath, "path": item.ArticlePath})
		}
		for _, item := range diff.Removed {
			removedJSON = append(removedJSON, map[string]any{"identity": item.Path, "path": item.Path})
		}
		details["added"], details["removed"] = addedJSON, removedJSON
	}
	prompt, err := ReconcilePrompts(project, dryRun)
	if err != nil {
		return result, warnings, err
	}
	thinking, err := ReconcileThinking(project, dryRun)
	if err != nil {
		return result, warnings, err
	}
	details["prompts"], details["thinking"] = roleIndexDetails(prompt.Added, prompt.Removed, prompt.KeptCount), roleIndexDetails(thinking.Added, thinking.Removed, thinking.KeptCount)
	result.Details = details
	return result, warnings, nil
}
