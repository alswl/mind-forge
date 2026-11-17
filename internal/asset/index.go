package asset

import (
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func scan(project, directory string) []string {
	files := map[string]bool{}
	active := map[string]bool{}
	var walk func(string)
	walk = func(name string) {
		canonical, err := filepath.EvalSymlinks(name)
		if err != nil || active[canonical] {
			return
		}
		info, err := os.Stat(name)
		if err != nil {
			return
		}
		if !info.IsDir() {
			if !info.Mode().IsRegular() || strings.HasPrefix(filepath.Base(name), ".") || filepath.Base(name) == "Thumbs.db" {
				return
			}
			rel, err := filepath.Rel(project, name)
			if err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
				files[strings.ReplaceAll(filepath.ToSlash(rel), "\\", "/")] = true
			}
			return
		}
		active[canonical] = true
		defer delete(active, canonical)
		entries, err := os.ReadDir(name)
		if err != nil {
			return
		}
		for _, entry := range entries {
			walk(filepath.Join(name, entry.Name()))
		}
	}
	walk(directory)
	result := make([]string, 0, len(files))
	for name := range files {
		result = append(result, name)
	}
	sort.Strings(result)
	return result
}

func (service Service) Reconcile(project string, dryRun, refreshMetadata bool) (model.AssetIndexReport, error) {
	report := model.AssetIndexReport{Added: model.JSONArray[model.AssetIndexEntry]{}, Removed: model.JSONArray[model.AssetIndexEntry]{}}
	layout, err := projectLayout(project)
	if err != nil {
		return report, err
	}
	directory := joinPath(project, layout.Assets)
	if _, err := os.Stat(directory); err != nil {
		return report, output.Usage("assets directory not found at '"+directory+"'", "run `mf project lint --fix` to create missing directories")
	}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	items := assetEntries(document.Value)
	indexed := map[string]bool{}
	names := map[string]string{}
	for _, item := range items {
		indexed[item.Path] = true
		if _, exists := names[item.Path]; !exists {
			names[item.Path] = item.Name
		}
	}
	if refreshMetadata {
		results := refreshEntries(project, items)
		if len(results) > 0 {
			report.Refreshed = &results
		}
	}
	canonical, err := filepath.EvalSymlinks(project)
	if err != nil {
		return report, err
	}
	relativeDirectory, err := filepath.Rel(project, directory)
	if err != nil {
		return report, err
	}
	paths := scan(canonical, filepath.Join(canonical, relativeDirectory))
	disk := map[string]bool{}
	for _, path := range paths {
		disk[path] = true
		if indexed[path] {
			report.KeptCount++
			continue
		}
		full := joinPath(project, path)
		size, hash, _ := Metadata(full)
		item := model.Asset{Name: assetFileName(path), Kind: InferKind(path), Path: path, Size: size, Hash: hash, Tags: model.JSONArray[string]{}, AddedAt: service.timestamp()}
		items = append(items, item)
		report.Added = append(report.Added, model.AssetIndexEntry{Name: item.Name, Path: path})
	}
	missing := []string{}
	for path := range indexed {
		if !disk[path] {
			missing = append(missing, path)
		}
	}
	sort.Strings(missing)
	for _, path := range missing {
		report.Removed = append(report.Removed, model.AssetIndexEntry{Name: names[path], Path: path})
	}
	kept := make([]model.Asset, 0, len(items))
	for _, item := range items {
		if disk[item.Path] {
			kept = append(kept, item)
		}
	}
	sort.SliceStable(kept, func(i, j int) bool { return kept[i].Name < kept[j].Name })
	sort.SliceStable(report.Added, func(i, j int) bool { return report.Added[i].Name < report.Added[j].Name })
	sort.SliceStable(report.Removed, func(i, j int) bool { return report.Removed[i].Name < report.Removed[j].Name })
	if !dryRun {
		updated := document.Value
		if updated.Assets != nil || len(kept) > 0 {
			updated.Assets = &kept
		}
		_, err = service.save(project, document, updated)
	}
	return report, err
}
