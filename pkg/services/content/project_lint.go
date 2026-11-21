package content

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

var ProjectLintRules = []string{"missing_directory", "duplicate_key", "stale_index_entry", "name_convention", "missing_manifest", "orphan_prompt", "duplicate_binding", "missing_thinking"}

type ProjectLintSummary struct {
	Errors   uint64 `json:"errors"`
	Warnings uint64 `json:"warnings"`
	Fixed    uint64 `json:"fixed"`
	Unfixed  uint64 `json:"unfixed"`
}

func NormalizeProjectLintRules(rules []string) ([]string, error) {
	if len(rules) == 0 {
		return append([]string(nil), ProjectLintRules...), nil
	}
	normalized := []string{}
	for _, rule := range rules {
		lowered := textcase.ASCIILower(rule)
		valid := false
		for _, known := range ProjectLintRules {
			valid = valid || lowered == known
		}
		if !valid {
			return nil, output.Usage("unknown lint rule '"+rule+"': invalid variant: "+rule, "available rules: missing_directory, stale_index_entry, name_convention, missing_manifest, duplicate_key, orphan_prompt, duplicate_binding, missing_thinking")
		}
		normalized = append(normalized, lowered)
	}
	return normalized, nil
}

func LintProject(project string, rules []string, fix bool) ([]model.ProjectLintIssue, ProjectLintSummary, error) {
	summary := ProjectLintSummary{}
	issues := []model.ProjectLintIssue{}
	rules, err := NormalizeProjectLintRules(rules)
	if err != nil {
		return issues, summary, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return issues, summary, err
	}
	layout, _ := app.ResolveEffectiveLayout(config)
	add := func(severity, kind, message, path string, fixable, fixed, unfixed bool) {
		issues = append(issues, model.ProjectLintIssue{Severity: severity, Kind: kind, Message: message, Path: path, Fixable: fixable, Fixed: fixed})
		if fixed {
			summary.Fixed++
		} else if unfixed {
			summary.Unfixed++
		}
	}
	indexPath := filepath.Join(project, "mind-index.yaml")
	for _, rule := range rules {
		switch rule {
		case "missing_directory":
			if config.ContentLayout != nil {
				continue
			}
			for _, directory := range []string{layout.Articles, layout.Sources, layout.Assets} {
				path := joinProjectPath(project, directory)
				if pathExists(path) {
					continue
				}
				fixed := fix && os.MkdirAll(filepath.Dir(path), 0o755) == nil &&
					store.CreateDirectory(path, map[string][]byte{".gitkeep": nil}) == nil
				add("error", rule, "missing directory '"+directory+"'", directory+"/", true, fixed, true)
			}
		case "missing_manifest":
			path := filepath.Join(project, "mind.yaml")
			if pathExists(path) {
				continue
			}
			fixed := fix && (store.AtomicWriter{}).Write(path, []byte("schema: '1'\n"), 0644) == nil
			add("error", rule, "missing mind.yaml", "mind.yaml", true, fixed, true)
		case "duplicate_key":
			raw, err := os.ReadFile(indexPath)
			if err != nil {
				continue
			}
			duplicates, err := store.IndexDuplicates(raw)
			if err != nil || len(duplicates) == 0 {
				continue
			}
			duplicate := duplicates[0]
			fixed := false
			if fix {
				cleaned, err := store.MergeDuplicateIndex(raw)
				fixed = err == nil && (store.AtomicWriter{}).Write(indexPath, cleaned, 0644) == nil
			}
			add("warning", rule, fmt.Sprintf("duplicate key '%s' at line %d", duplicate.Key, duplicate.Line), "mind-index.yaml", true, fixed, false)
		case "stale_index_entry":
			document, err := store.ReadIndex(indexPath, true)
			if err != nil {
				continue
			}
			stale := []string{}
			if document.Value.Articles != nil {
				for _, item := range *document.Value.Articles {
					if !pathExists(joinProjectPath(project, item.Path)) {
						stale = append(stale, item.Path)
					}
				}
			}
			if document.Value.Assets != nil {
				for _, item := range *document.Value.Assets {
					if !pathExists(joinProjectPath(project, item.Path)) {
						stale = append(stale, item.Path)
					}
				}
			}
			if document.Value.Sources != nil {
				for _, item := range *document.Value.Sources {
					if item.Path != nil && !pathExists(joinProjectPath(project, *item.Path)) {
						stale = append(stale, *item.Path)
					}
				}
			}
			for _, path := range stale {
				add("warning", rule, "index references '"+path+"' which does not exist", path, true, false, false)
			}
			if fix && len(stale) > 0 {
				remove := map[string]bool{}
				for _, path := range stale {
					remove[path] = true
				}
				updated := document.Value
				if updated.Articles != nil {
					items := []model.Article{}
					for _, item := range *updated.Articles {
						if !remove[item.Path] {
							items = append(items, item)
						}
					}
					updated.Articles = &items
				}
				if updated.Assets != nil {
					items := []model.Asset{}
					for _, item := range *updated.Assets {
						if !remove[item.Path] {
							items = append(items, item)
						}
					}
					updated.Assets = &items
				}
				if updated.Sources != nil {
					items := []model.Source{}
					for _, item := range *updated.Sources {
						if item.Path == nil || !remove[*item.Path] {
							items = append(items, item)
						}
					}
					updated.Sources = &items
				}
				if err := document.Write(indexPath, updated, false); err != nil {
					return issues, summary, err
				}
				for i := len(issues) - len(stale); i < len(issues); i++ {
					issues[i].Fixed = true
				}
				summary.Fixed += uint64(len(stale))
			}
		case "name_convention":
			document, err := store.ReadIndex(indexPath, true)
			if err != nil || document.Value.Articles == nil {
				continue
			}
			for _, item := range *document.Value.Articles {
				name := filepath.Base(item.Path)
				stem := strings.TrimSuffix(name, filepath.Ext(name))
				if ArticleFilename(stem) != stem {
					add("warning", rule, "filename '"+name+"' should be kebab-case", item.Path, false, false, false)
				}
			}
		case "orphan_prompt", "duplicate_binding", "missing_thinking":
			document, err := store.ReadIndex(indexPath, true)
			if err != nil {
				continue
			}
			index := document.Value
			prompts, thinking := articleBindings(index)
			if rule == "orphan_prompt" {
				for _, binding := range resolvePromptBindings(index) {
					if binding.Status == "orphan" {
						prompt := binding.Prompt
						article := prompt.Article
						if article == "" {
							article = "(none)"
						}
						add("error", rule, "prompt '"+prompt.Path+"' is bound to non-existent article '"+article+"'", prompt.Path, false, false, true)
					}
				}
			}
			if rule == "duplicate_binding" {
				paths := []string{}
				for path, view := range prompts {
					if view.BindingStatus == "duplicate" {
						paths = append(paths, path)
					}
				}
				sort.Strings(paths)
				for _, path := range paths {
					add("error", rule, "article '"+path+"' has multiple bound prompts: "+strings.Join(prompts[path].Conflicts, ", "), path, false, false, true)
				}
			}
			if rule == "missing_thinking" && index.Articles != nil {
				for _, article := range *index.Articles {
					prompt := prompts[article.Path]
					if prompt == nil || prompt.BindingStatus != "bound" {
						continue
					}
					key := ArticleOutputStem(article.Path)
					if thinking[key] == nil {
						add("warning", rule, "article '"+article.Path+"' has no thinking ledger (expected thinking/"+key+".md)", article.Path, false, false, true)
					}
				}
			}
		}
	}
	names := make([]string, 0, len(config.Build.Articles))
	for name := range config.Build.Articles {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		directory := config.Build.Articles[name].ArticleDir
		if directory != nil && !isDirectory(joinProjectPath(project, *directory)) {
			add("error", "missing_directory", "configured article directory '"+*directory+"' for article '"+name+"' does not exist", *directory+"/", true, false, true)
		}
	}
	for _, issue := range issues {
		if !issue.Fixed {
			if issue.Severity == "error" {
				summary.Errors++
			} else if issue.Severity == "warning" {
				summary.Warnings++
			}
		}
	}
	return issues, summary, nil
}

func LintRepository(root string, rules []string, fix bool) ([]map[string]any, ProjectLintSummary, error) {
	manifest, err := loadManifest(root)
	if err != nil {
		return nil, ProjectLintSummary{}, err
	}
	objects := []map[string]any{}
	total := ProjectLintSummary{}
	projects, err := ScanProjects(root, manifest.ProjectsDir)
	if err != nil {
		return nil, total, err
	}
	for _, entry := range projects {
		issues, summary, err := LintProject(joinProjectPath(root, strings.TrimPrefix(entry.Path, "./")), rules, fix)
		if err != nil {
			return nil, total, err
		}
		total.Errors += summary.Errors
		total.Warnings += summary.Warnings
		total.Fixed += summary.Fixed
		total.Unfixed += summary.Unfixed
		for _, issue := range issues {
			objects = append(objects, map[string]any{"severity": issue.Severity, "kind": issue.Kind, "message": issue.Message, "path": issue.Path, "fixable": issue.Fixable, "fixed": issue.Fixed, "project": entry.Name})
		}
	}
	return objects, total, nil
}
