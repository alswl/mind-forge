package content

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func exactArticle(index model.Index, selector, hint string) (model.Article, int, error) {
	if index.Articles != nil {
		for i, article := range *index.Articles {
			if article.Title == selector || article.Path == selector {
				return article, i, nil
			}
		}
	}
	return model.Article{}, -1, &output.Error{Kind: "not_found", Message: fmt.Sprintf("article '%s' not found", selector), Hint: &hint}
}

func PlanArticleRename(root, project, selector, newSlug string, force bool, now time.Time) (ContentMutation, error) {
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return ContentMutation{}, err
	}
	document, indexPath, err := inventory.index(project)
	if err != nil {
		return ContentMutation{}, err
	}
	updated := document.Value
	article, position, err := exactArticle(updated, selector, "use `mf article list --project <project>` to see available articles")
	if err != nil {
		return ContentMutation{}, err
	}
	source, err := inventory.projectPath(project, article.Path)
	if err != nil {
		return ContentMutation{}, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return ContentMutation{}, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return ContentMutation{}, err
	}
	slug := ArticleFilename(newSlug)
	relative := layout.Articles + "/" + slug
	entry := inventory.before[source]
	if entry.Kind != "directory" {
		relative += ".md"
	}
	return planArticleRename(inventory, document, indexPath, project, position, source, relative, force, false, now)
}

func planLintArticleRename(ctx context.Context, root, project, oldPath, newPath string) (ContentMutation, error) {
	inventory, err := readContentInventory(ctx, root)
	if err != nil {
		return ContentMutation{}, err
	}
	document, indexPath, err := inventory.index(project)
	if err != nil {
		return ContentMutation{}, err
	}
	position := -1
	if document.Value.Articles != nil {
		for i, article := range *document.Value.Articles {
			path := filepath.ToSlash(filepath.Clean(article.Path))
			if path == oldPath {
				position = i
			}
			if strings.TrimSuffix(path, ".md") == strings.TrimSuffix(newPath, ".md") {
				return ContentMutation{}, fileExists(filepath.Join(project, newPath))
			}
		}
	}
	if position < 0 {
		return ContentMutation{}, output.Usage("article registration changed during lint: "+oldPath, "rerun article lint")
	}
	source, err := inventory.projectPath(project, oldPath)
	if err != nil {
		return ContentMutation{}, err
	}
	return planArticleRename(inventory, document, indexPath, project, position, source, newPath, false, true, time.Time{})
}

func planArticleRename(inventory *contentInventory, document *store.IndexDocument, indexPath, project string, position int, source, relative string, force, keepBindings bool, now time.Time) (ContentMutation, error) {
	updated := document.Value
	article := (*updated.Articles)[position]
	_, sourceExists := inventory.before[source]
	slug := strings.TrimSuffix(filepath.Base(relative), ".md")
	target, err := inventory.projectPath(project, relative)
	if err != nil {
		return ContentMutation{}, err
	}
	relocations := map[string]string{}
	if article.Path != relative {
		if !sourceExists {
			hint := "the index may be out of date; try `mf article index`"
			return ContentMutation{}, &output.Error{Kind: "not_found", Message: "article not found at " + filepath.Join(project, article.Path), Hint: &hint}
		}
		if err := inventory.relocate(source, target, force); err != nil {
			return ContentMutation{}, err
		}
		for _, role := range []string{"prompts", "thinking"} {
			oldRelative, newRelative := role+"/"+ArticleOutputStem(article.Path)+".md", role+"/"+slug+".md"
			oldPath, err := inventory.projectPath(project, oldRelative)
			if err != nil {
				return ContentMutation{}, err
			}
			newPath, err := inventory.projectPath(project, newRelative)
			if err != nil {
				return ContentMutation{}, err
			}
			if _, exists := inventory.after[oldPath]; !exists {
				continue
			}
			if _, exists := inventory.after[newPath]; exists && !force {
				if keepBindings {
					return ContentMutation{}, fileExists(filepath.Join(project, newRelative))
				}
				continue
			}
			if err := inventory.relocate(oldPath, newPath, force); err != nil {
				return ContentMutation{}, err
			}
			relocations[newPath] = oldPath
			if role == "prompts" {
				prompt := inventory.after[newPath]
				if utf8.Valid(prompt.Bytes) {
					prompt.Bytes = []byte(updatePromptArticleBinding(string(prompt.Bytes), article.Path, relative))
					inventory.after[newPath] = prompt
				}
			}
		}
	}
	if keepBindings {
		prompts, err := inventory.projectPath(project, "prompts")
		if err != nil {
			return ContentMutation{}, err
		}
		for path, entry := range inventory.after {
			if entry.Kind != "file" || filepath.ToSlash(filepath.Dir(path)) != prompts ||
				strings.HasPrefix(filepath.Base(path), ".") || filepath.Ext(path) != ".md" || !utf8.Valid(entry.Bytes) {
				continue
			}
			rebound := updatePromptArticleBinding(string(entry.Bytes), article.Path, relative)
			if rebound != string(entry.Bytes) {
				entry.Bytes = []byte(rebound)
				inventory.after[path] = entry
			}
		}
	}
	oldPath := article.Path
	article.Path, article.UpdatedAt = relative, articleTimestamp(now)
	articles := append([]model.Article(nil), (*updated.Articles)...)
	articles[position] = article
	updated.Articles = &articles
	refreshPlannedRoles(inventory, project, &updated, relocations)
	if err := inventory.setIndex(document, indexPath, updated); err != nil {
		return ContentMutation{}, err
	}
	result := output.VerbResult{Verb: output.Rename, Kind: "article", Identity: relative, OldIdentity: &oldPath, Path: &relative, Details: map[string]any{"old_title": article.Title, "new_title": article.Title}}
	return inventory.finish("article rename", result)
}

func updatePromptArticleBinding(content, oldPath, newPath string) string {
	_, body, _, ok := markdown.InitialFrontMatter(content)
	if !ok {
		return content
	}
	front := content[:len(content)-len(body)]
	lines := strings.SplitAfter(front, "\n")
	for i, line := range lines {
		rest, ok := strings.CutPrefix(line, "article:")
		if !ok || strings.Trim(strings.TrimSpace(rest), "\"'") != oldPath {
			continue
		}
		eol := line[len(strings.TrimRight(line, "\r\n")):]
		lines[i] = "article: " + newPath + eol
	}
	return strings.Join(lines, "") + body
}

func refreshPlannedRoles(inventory *contentInventory, project string, index *model.Index, moved map[string]string) {
	prefix, err := filepath.Rel(inventory.root, project)
	if err != nil {
		return
	}
	prefix = filepath.ToSlash(prefix)
	if prefix == "." {
		prefix = ""
	} else {
		prefix += "/"
	}
	previousPrompts, previousThinking := map[string]string{}, map[string]string{}
	if index.Prompts != nil {
		for _, item := range *index.Prompts {
			previousPrompts[item.Path] = item.UpdatedAt
		}
	}
	if index.Thinking != nil {
		for _, item := range *index.Thinking {
			previousThinking[item.Path] = item.UpdatedAt
		}
	}
	articles := map[string]string{}
	if index.Articles != nil {
		for _, article := range *index.Articles {
			articles[ArticleOutputStem(article.Path)] = article.Path
		}
	}
	names := make([]string, 0, len(inventory.after))
	for name := range inventory.after {
		names = append(names, name)
	}
	sort.Strings(names)
	prompts, thoughts := []model.Prompt{}, []model.Thinking{}
	promptValid, thinkingValid := true, true
	for _, name := range names {
		entry := inventory.after[name]
		if entry.Kind != "file" {
			continue
		}
		relative, inside := strings.CutPrefix(name, prefix)
		if !inside {
			continue
		}
		role, filename, ok := strings.Cut(relative, "/")
		if !ok || strings.Contains(filename, "/") || strings.HasPrefix(filename, ".") || !strings.HasSuffix(filename, ".md") {
			continue
		}
		if role != "prompts" && role != "thinking" {
			continue
		}
		prior := previousPrompts
		if role == "thinking" {
			prior = previousThinking
		}
		stamp, kept := prior[relative]
		if !kept {
			original := name
			if from, ok := moved[name]; ok {
				original = from
			}
			info, err := os.Stat(filepath.Join(inventory.root, original))
			if err != nil {
				if role == "prompts" {
					promptValid = false
				} else {
					thinkingValid = false
				}
				continue
			}
			stamp = articleTimestamp(info.ModTime())
		}
		if role == "prompts" {
			if !utf8.Valid(entry.Bytes) {
				promptValid = false
				continue
			}
			article, mode := ParsePromptFrontMatter(string(entry.Bytes))
			prompts = append(prompts, model.Prompt{Path: relative, Article: article, Mode: mode, UpdatedAt: stamp})
		} else {
			key := strings.TrimSuffix(filename, ".md")
			thoughts = append(thoughts, model.Thinking{Path: relative, Article: articles[key], UpdatedAt: stamp})
		}
	}
	if promptValid {
		index.Prompts = &prompts
	}
	if thinkingValid {
		index.Thinking = &thoughts
	}
}

func PlanArticleRemove(root, project, selector string, dryRun bool) (ContentMutation, error) {
	if strings.TrimSpace(selector) == "" {
		return ContentMutation{}, output.Usage("article title must not be empty", "")
	}
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return ContentMutation{}, err
	}
	document, indexPath, err := inventory.index(project)
	if err != nil {
		return ContentMutation{}, err
	}
	path, err := resolveArticleEntries(document.Value, selector)
	if err != nil {
		return ContentMutation{}, err
	}
	source, err := inventory.projectPath(project, path)
	if err != nil {
		return ContentMutation{}, err
	}
	eraseContentTree(inventory.after, source)
	updated := document.Value
	articles := []model.Article{}
	for _, article := range *updated.Articles {
		if article.Path != path {
			articles = append(articles, article)
		}
	}
	updated.Articles = &articles
	if err := inventory.setIndex(document, indexPath, updated); err != nil {
		return ContentMutation{}, err
	}
	result := output.VerbResult{Verb: output.Remove, Kind: "article", Identity: selector, Path: &path, DryRun: dryRun, Details: map[string]any{"removed": true}}
	return inventory.finish("article remove", result)
}

func PlanArticleMove(root, sourceProject, targetProject, selector string, dryRun bool) (ContentMutation, error) {
	if canonicalPath(sourceProject) == canonicalPath(targetProject) {
		return ContentMutation{}, output.Usage("article source and destination projects must differ", "")
	}
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return ContentMutation{}, err
	}
	sourceDocument, sourceIndexPath, err := inventory.index(sourceProject)
	if err != nil {
		return ContentMutation{}, err
	}
	targetDocument, targetIndexPath, err := inventory.index(targetProject)
	if err != nil {
		return ContentMutation{}, err
	}
	sourceIndex, targetIndex := sourceDocument.Value, targetDocument.Value
	article, position, err := exactArticle(sourceIndex, selector, "use `mf article list` to see available articles")
	if err != nil {
		return ContentMutation{}, err
	}
	oldPath := article.Path
	source, err := inventory.projectPath(sourceProject, oldPath)
	if err != nil {
		return ContentMutation{}, err
	}
	target, err := inventory.projectPath(targetProject, oldPath)
	if err != nil {
		return ContentMutation{}, err
	}
	if _, exists := inventory.before[source]; !dryRun && !exists {
		hint := "run `mf article index` to refresh the source project"
		return ContentMutation{}, &output.Error{Kind: "not_found", Message: "article path '" + filepath.Join(sourceProject, oldPath) + "' not found", Hint: &hint}
	}
	if targetIndex.Articles != nil {
		for _, item := range *targetIndex.Articles {
			if item.Path == oldPath {
				return ContentMutation{}, output.Usage("destination already contains article '"+oldPath+"'", "rename or remove the destination article before moving")
			}
		}
	}
	prompts, remainingPrompts := []model.Prompt{}, []model.Prompt{}
	if sourceIndex.Prompts != nil {
		for _, item := range *sourceIndex.Prompts {
			if item.Article == oldPath {
				prompts = append(prompts, item)
			} else {
				remainingPrompts = append(remainingPrompts, item)
			}
		}
	}
	thoughts, remainingThoughts := []model.Thinking{}, []model.Thinking{}
	if sourceIndex.Thinking != nil {
		for _, item := range *sourceIndex.Thinking {
			if item.Article == oldPath {
				thoughts = append(thoughts, item)
			} else {
				remainingThoughts = append(remainingThoughts, item)
			}
		}
	}
	records, remainingRecords := []model.PublishRecord{}, []model.PublishRecord{}
	if sourceIndex.PublishRecords != nil {
		for _, item := range *sourceIndex.PublishRecords {
			if item.Path == oldPath {
				records = append(records, item)
			} else {
				remainingRecords = append(remainingRecords, item)
			}
		}
	}
	movedPrompts, movedThinking := []string{}, []string{}
	for _, item := range prompts {
		if targetIndex.Prompts != nil {
			for _, candidate := range *targetIndex.Prompts {
				if candidate.Path == item.Path {
					return ContentMutation{}, output.Usage("destination already contains prompt '"+item.Path+"'", "")
				}
			}
		}
		movedPrompts = append(movedPrompts, item.Path)
	}
	for _, item := range thoughts {
		if targetIndex.Thinking != nil {
			for _, candidate := range *targetIndex.Thinking {
				if candidate.Path == item.Path {
					return ContentMutation{}, output.Usage("destination already contains thinking entry '"+item.Path+"'", "")
				}
			}
		}
		movedThinking = append(movedThinking, item.Path)
	}
	result := output.VerbResult{Verb: output.Move, Kind: "article", Identity: oldPath, OldIdentity: &oldPath, DryRun: dryRun, Details: map[string]any{"old_path": oldPath, "new_path": oldPath, "moved_prompts": movedPrompts, "moved_thinking": movedThinking, "moved_publish_records": len(records), "from_scope": sourceProject, "to_scope": targetProject}}
	if dryRun {
		return inventory.finish("article move", result)
	}
	result.Path = &oldPath
	if _, exists := inventory.before[target]; exists {
		return ContentMutation{}, output.Usage("destination article path '"+oldPath+"' already exists", "remove the destination path before moving")
	}
	if err := inventory.relocate(source, target, false); err != nil {
		return ContentMutation{}, err
	}
	for _, relative := range append(append([]string{}, movedPrompts...), movedThinking...) {
		from, err := inventory.projectPath(sourceProject, relative)
		if err != nil {
			return ContentMutation{}, err
		}
		to, err := inventory.projectPath(targetProject, relative)
		if err != nil {
			return ContentMutation{}, err
		}
		if _, exists := inventory.before[from]; !exists {
			continue
		}
		if err := inventory.relocate(from, to, false); err != nil {
			return ContentMutation{}, err
		}
	}
	articles := append([]model.Article(nil), (*sourceIndex.Articles)...)
	articles = append(articles[:position], articles[position+1:]...)
	sourceIndex.Articles = &articles
	if sourceIndex.Prompts != nil {
		sourceIndex.Prompts = &remainingPrompts
	}
	if sourceIndex.Thinking != nil {
		sourceIndex.Thinking = &remainingThoughts
	}
	if sourceIndex.PublishRecords != nil {
		sourceIndex.PublishRecords = &remainingRecords
	}
	targetArticles, targetPrompts, targetThoughts, targetRecords := []model.Article{}, []model.Prompt{}, []model.Thinking{}, []model.PublishRecord{}
	if targetIndex.Articles != nil {
		targetArticles = append(targetArticles, (*targetIndex.Articles)...)
	}
	if targetIndex.Prompts != nil {
		targetPrompts = append(targetPrompts, (*targetIndex.Prompts)...)
	}
	if targetIndex.Thinking != nil {
		targetThoughts = append(targetThoughts, (*targetIndex.Thinking)...)
	}
	if targetIndex.PublishRecords != nil {
		targetRecords = append(targetRecords, (*targetIndex.PublishRecords)...)
	}
	targetArticles = append(targetArticles, article)
	targetPrompts = append(targetPrompts, prompts...)
	targetThoughts = append(targetThoughts, thoughts...)
	targetRecords = append(targetRecords, records...)
	targetIndex.Articles, targetIndex.Prompts, targetIndex.Thinking, targetIndex.PublishRecords = &targetArticles, &targetPrompts, &targetThoughts, &targetRecords
	if err := inventory.setIndex(sourceDocument, sourceIndexPath, sourceIndex); err != nil {
		return ContentMutation{}, err
	}
	if err := inventory.setIndex(targetDocument, targetIndexPath, targetIndex); err != nil {
		return ContentMutation{}, err
	}
	return inventory.finish("article move", result)
}
