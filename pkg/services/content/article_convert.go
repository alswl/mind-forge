package content

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/managers/lifecycle"
)

type ConversionInspection struct {
	Eligible                                                                             bool
	Direction, SourceShape, SourcePath, SourceContentPath, TargetPath, TargetContentPath string
	Reason                                                                               *string
	SectionFiles                                                                         []string
}

func (inspection ConversionInspection) Result(status string, indexUpdated, sourceRemoved bool) model.ConversionResult {
	shape := "directory"
	if inspection.Direction == "to_single_file" {
		shape = "single_file"
	}
	result := model.ConversionResult{Status: status, Direction: inspection.Direction, SourceShape: inspection.SourceShape, TargetShape: shape, SourcePath: inspection.SourcePath, SourceContentPath: inspection.SourceContentPath, TargetPath: inspection.TargetPath, TargetContentPath: inspection.TargetContentPath, Reason: inspection.Reason, IndexUpdated: indexUpdated, SourceRemoved: sourceRemoved}
	if len(inspection.SectionFiles) > 1 {
		files := model.JSONArray[string](append([]string(nil), inspection.SectionFiles...))
		result.MergedSectionFiles = &files
	}
	return result
}

func InspectArticleConversions(root, project string, paths []string, direction string, merge bool) ([]ConversionInspection, error) {
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return nil, err
	}
	items := make([]ConversionInspection, 0, len(paths))
	for _, path := range paths {
		item, err := inspectArticleConversion(inventory, project, path, direction, merge)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].SourcePath < items[j].SourcePath })
	return items, nil
}

func inspectArticleConversion(inventory *contentInventory, project, path, direction string, merge bool) (ConversionInspection, error) {
	item := ConversionInspection{Direction: direction, SourcePath: path, SourceContentPath: path, SourceShape: "single_file"}
	source, err := inventory.projectPath(project, path)
	if err != nil {
		return item, err
	}
	if entry, ok := inventory.before[source]; ok && entry.Kind == "directory" {
		item.SourceShape = "directory"
	}
	if direction == "to_single_file" {
		item.TargetPath = strings.TrimRight(path, "/") + ".md"
		item.TargetContentPath = item.TargetPath
	} else if direction == "to_directory" {
		item.TargetPath = strings.TrimSuffix(path, ".md")
		item.TargetContentPath = item.TargetPath + "/01-opening.md"
	} else {
		return item, output.Usage("unknown conversion direction", "")
	}
	target, err := inventory.projectPath(project, item.TargetPath)
	if err != nil {
		return item, err
	}
	skip := func(reason string) (ConversionInspection, error) { item.Reason = &reason; return item, nil }
	if direction == "to_directory" {
		if item.SourceShape != "single_file" {
			return skip("not_single_file_article")
		}
		if _, exists := inventory.before[target]; exists {
			return skip("target_exists")
		}
		item.Eligible = true
		return item, nil
	}
	if item.SourceShape != "directory" {
		return skip("not_directory_article")
	}
	sections := []string{}
	extras := false
	for name := range inventory.before {
		rest, ok := strings.CutPrefix(name, source+"/")
		if !ok || strings.Contains(rest, "/") {
			continue
		}
		if filepath.Ext(rest) == ".md" {
			sections = append(sections, path+"/"+rest)
		} else {
			extras = true
		}
	}
	sort.Strings(sections)
	if len(sections) == 0 {
		return skip("no_section_files")
	}
	if len(sections) > 1 && !merge {
		return skip("multiple_section_files")
	}
	item.SourceContentPath = sections[0]
	if _, exists := inventory.before[target]; exists {
		return skip("target_exists")
	}
	if extras {
		return skip("extra_files")
	}
	item.SectionFiles, item.Eligible = sections, true
	return item, nil
}

func PlanArticleConversion(root, project string, inspection ConversionInspection, now time.Time) (ContentMutation, error) {
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return ContentMutation{}, err
	}
	current, err := inspectArticleConversion(inventory, project, inspection.SourcePath, inspection.Direction, len(inspection.SectionFiles) > 1)
	if err != nil {
		return ContentMutation{}, err
	}
	if !current.Eligible {
		return ContentMutation{}, output.Usage("article is no longer eligible for conversion", "")
	}
	inspection = current
	source, err := inventory.projectPath(project, inspection.SourcePath)
	if err != nil {
		return ContentMutation{}, err
	}
	target, err := inventory.projectPath(project, inspection.TargetContentPath)
	if err != nil {
		return ContentMutation{}, err
	}
	read := func(relative string) (lifecycle.Entry, error) {
		path, err := inventory.projectPath(project, relative)
		if err != nil {
			return lifecycle.Entry{}, err
		}
		entry, ok := inventory.before[path]
		if !ok {
			return entry, &output.Error{Kind: "io", Message: "article content not found: " + relative}
		}
		if entry.Kind != "file" {
			return entry, &output.Error{Kind: "io", Message: "article content is not a file: " + relative}
		}
		if !utf8.Valid(entry.Bytes) {
			return entry, &output.Error{Kind: "io", Message: "stream did not contain valid UTF-8: " + relative}
		}
		return entry, nil
	}
	first, err := read(inspection.SourceContentPath)
	if err != nil {
		return ContentMutation{}, err
	}
	body := first.Bytes
	if len(inspection.SectionFiles) > 1 {
		var merged strings.Builder
		for _, section := range inspection.SectionFiles {
			entry, err := read(section)
			if err != nil {
				return ContentMutation{}, err
			}
			text := markdown.StripTyporaFrontMatter(string(entry.Bytes))
			merged.WriteString(text)
			if !strings.HasSuffix(merged.String(), "\n") {
				merged.WriteByte('\n')
			}
		}
		sourceDir := strings.TrimRight(inspection.SourcePath, "/")
		baseDir := filepath.Dir(sourceDir)
		body = []byte(markdown.RewriteReferences(merged.String(), func(target string) (string, bool) { return markdown.RebaseRelativeTarget(target, sourceDir, baseDir) }))
	}
	eraseContentTree(inventory.after, source)
	inventory.after[target] = lifecycle.Entry{Kind: "file", Mode: first.Mode, Bytes: body}
	document, indexPath, err := inventory.index(project)
	if err != nil {
		return ContentMutation{}, err
	}
	updated := document.Value
	if updated.Articles != nil {
		articles := append([]model.Article(nil), (*updated.Articles)...)
		for i := range articles {
			if articles[i].Path == inspection.SourcePath {
				articles[i].Path = inspection.TargetPath
				articles[i].UpdatedAt = articleTimestamp(now)
				break
			}
		}
		updated.Articles = &articles
	}
	promptPath, err := inventory.projectPath(project, "prompts/"+ArticleOutputStem(inspection.SourcePath)+".md")
	if err != nil {
		return ContentMutation{}, err
	}
	if prompt, exists := inventory.after[promptPath]; exists {
		if prompt.Kind == "file" && utf8.Valid(prompt.Bytes) {
			prompt.Bytes = []byte(updatePromptArticleBinding(string(prompt.Bytes), inspection.SourcePath, inspection.TargetPath))
			inventory.after[promptPath] = prompt
			refreshPlannedRoles(inventory, project, &updated, nil)
		}
	}
	if err := inventory.setIndex(document, indexPath, updated); err != nil {
		return ContentMutation{}, fmt.Errorf("index update failed: %w", err)
	}
	return inventory.finish("article convert", output.VerbResult{Kind: "article", Identity: inspection.TargetPath})
}
