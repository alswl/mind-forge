package content

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/store"
)

func roleMarkdownPaths(project, role string) ([]string, error) {
	path, err := store.CanonicalWithin(project, filepath.Join(project, role))
	if err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(path)
	if os.IsNotExist(err) {
		return []string{}, nil
	}
	if err != nil {
		return nil, err
	}
	paths := []string{}
	for _, entry := range entries {
		if !strings.HasPrefix(entry.Name(), ".") && utf8.ValidString(entry.Name()) && filepath.Ext(entry.Name()) == ".md" && isRegularFile(filepath.Join(path, entry.Name())) {
			paths = append(paths, role+"/"+entry.Name())
		}
	}
	return paths, nil
}

func ParsePromptFrontMatter(content string) (string, *string) {
	lines := strings.Split(strings.ReplaceAll(content, "\r\n", "\n"), "\n")
	if len(lines) == 0 || lines[0] != "---" {
		return "", nil
	}
	article := ""
	var mode *string
	for _, line := range lines[1:] {
		if strings.TrimSpace(line) == "---" {
			break
		}
		if rest, ok := strings.CutPrefix(line, "article:"); ok {
			value := strings.Trim(strings.TrimSpace(rest), "\"'")
			if value != "" {
				article = value
			}
		} else if rest, ok := strings.CutPrefix(line, "mode:"); ok {
			value := strings.Trim(strings.TrimSpace(rest), "\"'")
			mode = nil
			value = strings.TrimSpace(value)
			if value == "editorial" || value == "research" || value == "decision-research" {
				mode = &value
			}
		}
	}
	return article, mode
}

func ReconcilePrompts(project string, dryRun bool) (model.PromptIndexReport, error) {
	report := model.PromptIndexReport{DryRun: dryRun, Added: model.JSONArray[model.Prompt]{}, Removed: model.JSONArray[model.Prompt]{}}
	path := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(path, false)
	if err != nil {
		return report, err
	}
	previous := map[string]string{}
	if document.Value.Prompts != nil {
		for _, entry := range *document.Value.Prompts {
			previous[entry.Path] = entry.UpdatedAt
		}
	}
	paths, err := roleMarkdownPaths(project, "prompts")
	if err != nil {
		return report, err
	}
	found := map[string]bool{}
	entries := []model.Prompt{}
	for _, relative := range paths {
		full, err := store.CanonicalWithin(project, filepath.Join(project, relative))
		if err != nil {
			return report, err
		}
		body, err := os.ReadFile(full)
		if err != nil {
			return report, err
		}
		if !utf8.Valid(body) {
			return report, fmt.Errorf("stream did not contain valid UTF-8: %s", full)
		}
		article, mode := ParsePromptFrontMatter(string(body))
		timestamp, kept := previous[relative]
		if kept {
			report.KeptCount++
		} else {
			info, err := os.Stat(full)
			if err != nil {
				return report, err
			}
			timestamp = articleTimestamp(info.ModTime())
		}
		entry := model.Prompt{Path: relative, Article: article, Mode: mode, UpdatedAt: timestamp}
		if !kept {
			report.Added = append(report.Added, entry)
		}
		entries = append(entries, entry)
		found[relative] = true
	}
	if document.Value.Prompts != nil {
		for _, entry := range *document.Value.Prompts {
			if !found[entry.Path] {
				report.Removed = append(report.Removed, entry)
			}
		}
	}
	if !dryRun {
		updated := document.Value
		updated.Prompts = &entries
		err = document.Write(path, updated, false)
	}
	return report, err
}

func ReconcileThinking(project string, dryRun bool) (model.ThinkingIndexReport, error) {
	report := model.ThinkingIndexReport{DryRun: dryRun, Added: model.JSONArray[model.Thinking]{}, Removed: model.JSONArray[model.Thinking]{}}
	path := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(path, false)
	if err != nil {
		return report, err
	}
	previous := map[string]string{}
	if document.Value.Thinking != nil {
		for _, entry := range *document.Value.Thinking {
			previous[entry.Path] = entry.UpdatedAt
		}
	}
	byKey := map[string]string{}
	if document.Value.Articles != nil {
		for _, article := range *document.Value.Articles {
			byKey[ArticleOutputStem(article.Path)] = article.Path
		}
	}
	paths, err := roleMarkdownPaths(project, "thinking")
	if err != nil {
		return report, err
	}
	found := map[string]bool{}
	entries := []model.Thinking{}
	for _, relative := range paths {
		full, err := store.CanonicalWithin(project, filepath.Join(project, relative))
		if err != nil {
			return report, err
		}
		timestamp, kept := previous[relative]
		if kept {
			report.KeptCount++
		} else {
			info, err := os.Stat(full)
			if err != nil {
				return report, err
			}
			timestamp = articleTimestamp(info.ModTime())
		}
		name := filepath.Base(relative)
		key := strings.TrimSuffix(name, filepath.Ext(name))
		entry := model.Thinking{Path: relative, Article: byKey[key], UpdatedAt: timestamp}
		if !kept {
			report.Added = append(report.Added, entry)
		}
		entries = append(entries, entry)
		found[relative] = true
	}
	if document.Value.Thinking != nil {
		for _, entry := range *document.Value.Thinking {
			if !found[entry.Path] {
				report.Removed = append(report.Removed, entry)
			}
		}
	}
	if !dryRun {
		updated := document.Value
		updated.Thinking = &entries
		err = document.Write(path, updated, false)
	}
	return report, err
}

func PromptProjection(project string) (model.JSONArray[model.Prompt], error) {
	document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return nil, err
	}
	items := model.JSONArray[model.Prompt]{}
	if document.Value.Prompts != nil {
		items = append(items, (*document.Value.Prompts)...)
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].Path < items[j].Path })
	return items, nil
}

func ThinkingProjection(project string) (model.JSONArray[model.Thinking], error) {
	document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return nil, err
	}
	items := model.JSONArray[model.Thinking]{}
	if document.Value.Thinking != nil {
		items = append(items, (*document.Value.Thinking)...)
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].Path < items[j].Path })
	return items, nil
}
