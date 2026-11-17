package content

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type ScannedProject struct{ Name, Path string }

func ScanProjects(root, container string) []ScannedProject {
	directory := strings.Trim(container, "/")
	base := root
	if directory != "" && directory != "." {
		base = filepath.Join(root, directory)
	}
	entries, err := os.ReadDir(base)
	if err != nil {
		return []ScannedProject{}
	}
	result := []ScannedProject{}
	for _, entry := range entries {
		path := filepath.Join(base, entry.Name())
		if isDirectory(path) {
			if _, err := os.Stat(filepath.Join(path, "mind.yaml")); err == nil {
				result = append(result, ScannedProject{Name: entry.Name(), Path: store.ProjectRelativePath(container, entry.Name())})
			}
		}
	}
	return result
}

func ReconcileProjectDocs(project string, dryRun bool, now time.Time) ([]string, []string, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, nil, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return nil, nil, err
	}
	scanned, err := ScanArticleDocs(project, config)
	if err != nil {
		return nil, nil, err
	}
	path := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(path, false)
	if err != nil {
		return nil, nil, err
	}
	diff := ArticleDiff(document.Value, scanned, layout.Articles)
	added, removed := []string{}, []string{}
	remove := map[string]bool{}
	for _, item := range diff.Added {
		added = append(added, scannedArticlePath(item, layout.Articles))
	}
	for _, item := range diff.Removed {
		if !pathExists(joinProjectPath(project, item.Path)) {
			removed = append(removed, item.Path)
			remove[item.Path] = true
		}
	}
	if !dryRun && (len(added) > 0 || len(removed) > 0) {
		items := []model.Article{}
		if document.Value.Articles != nil {
			for _, item := range *document.Value.Articles {
				if !remove[item.Path] {
					items = append(items, item)
				}
			}
		}
		for _, item := range diff.Added {
			items = append(items, freshArticle(project, scannedArticlePath(item, layout.Articles), item.Title, articleTimestamp(now)))
		}
		updated := document.Value
		updated.Articles = &items
		if err := document.Write(path, updated, false); err != nil {
			return nil, nil, err
		}
	}
	return added, removed, nil
}

func joinProjectPath(root, path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(root, path)
}
func pathExists(path string) bool { _, err := os.Stat(path); return err == nil }

func IndexProjects(root string, dryRun bool, now time.Time) (output.VerbResult, []string, error) {
	result := output.VerbResult{Verb: output.Index, Kind: "project", DryRun: dryRun, Details: map[string]any{}}
	manifest, err := loadManifest(root)
	if err != nil {
		return result, nil, err
	}
	scanned := ScanProjects(root, manifest.ProjectsDir)
	for _, entry := range manifest.Projects {
		project := joinProjectPath(root, strings.TrimPrefix(entry.Path, "./"))
		if entry.ArchivedAt != nil || !isRegularFile(filepath.Join(project, "mind.yaml")) {
			continue
		}
		config, err := ConfigFor(project)
		if err != nil {
			return result, nil, err
		}
		if config.ContentLayout == nil {
			continue
		}
		exists := false
		for _, item := range scanned {
			exists = exists || strings.TrimPrefix(item.Path, "./") == strings.TrimPrefix(entry.Path, "./")
		}
		if !exists {
			scanned = append(scanned, ScannedProject{Name: entry.Name, Path: entry.Path})
		}
	}
	changes := compareProjects(manifest, scanned, now)
	addedProjects, removedProjects := changes.added, changes.removed
	articleAdded, articleRemoved := []any{}, []any{}
	warnings := []string{}
	for _, entry := range scanned {
		project := joinProjectPath(root, strings.TrimPrefix(entry.Path, "./"))
		config, err := ConfigFor(project)
		if err != nil {
			return result, warnings, err
		}
		if config.ContentLayout != nil {
			if _, err := ReconcileContent(root, project, dryRun); err != nil {
				return result, warnings, err
			}
			continue
		}
		added, removed, err := ReconcileProjectDocs(project, dryRun, now)
		if err != nil {
			warnings = append(warnings, "skipped article reconcile for project '"+entry.Name+"': "+err.Error())
			continue
		}
		for _, path := range added {
			articleAdded = append(articleAdded, map[string]any{"identity": path, "path": path, "project": entry.Name})
		}
		for _, path := range removed {
			articleRemoved = append(articleRemoved, map[string]any{"identity": path, "path": path, "project": entry.Name})
		}
	}
	added, removed := []any{}, []any{}
	kept := len(manifest.Projects) - len(removedProjects)
	if dryRun {
		for _, entry := range addedProjects {
			added = append(added, entry)
		}
		for _, entry := range removedProjects {
			removed = append(removed, entry)
		}
	} else {
		manifest = reconcileProjects(manifest, changes)
		entries := manifest.Projects
		if err := store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest); err != nil {
			return result, warnings, err
		}
		for _, entry := range entries {
			added = append(added, map[string]any{"identity": entry.Name, "path": entry.Path})
		}
		kept = len(entries)
	}
	added = append(added, articleAdded...)
	removed = append(removed, articleRemoved...)
	result.Details = map[string]any{"added": added, "removed": removed, "kept_count": kept, "scanned_count": len(scanned), "minds_path": filepath.Join(root, "minds.yaml")}
	return result, warnings, nil
}
