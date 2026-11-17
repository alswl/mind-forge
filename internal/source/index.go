package source

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

type DiskSource struct {
	Path       string
	SourceKind *string
}

func Scan(project string) ([]DiskSource, error) {
	layout, err := projectLayout(project)
	if err != nil {
		return nil, err
	}
	base := filepath.Join(project, layout.Sources)
	if _, err := os.Stat(base); os.IsNotExist(err) {
		return []DiskSource{}, nil
	} else if err != nil {
		return nil, err
	}
	files := []DiskSource{}
	active := map[string]bool{}
	var walk func(string) error
	walk = func(directory string) error {
		canonical, err := filepath.EvalSymlinks(directory)
		if err != nil {
			return err
		}
		if active[canonical] {
			return fmt.Errorf("filesystem walk error: directory loop at %s", directory)
		}
		active[canonical] = true
		defer delete(active, canonical)
		entries, err := os.ReadDir(directory)
		if err != nil {
			return err
		}
		for _, entry := range entries {
			name := filepath.Join(directory, entry.Name())
			info, err := os.Stat(name)
			if err != nil {
				return fmt.Errorf("filesystem walk error: %w", err)
			}
			if info.IsDir() {
				if err := walk(name); err != nil {
					return err
				}
				continue
			}
			if strings.HasPrefix(entry.Name(), ".") || entry.Name() == "Thumbs.db" {
				continue
			}
			if !info.Mode().IsRegular() && entry.Type()&os.ModeSymlink == 0 {
				continue
			}
			rel, err := filepath.Rel(project, name)
			if err != nil {
				return err
			}
			item := DiskSource{Path: filepath.ToSlash(rel)}
			local, err := filepath.Rel(base, name)
			if err != nil {
				return err
			}
			first, _, _ := strings.Cut(filepath.ToSlash(local), "/")
			for _, kind := range []string{"yuque", "meeting", "misc"} {
				if first == kind {
					value := kind
					item.SourceKind = &value
				}
			}
			files = append(files, item)
		}
		return nil
	}
	if err := walk(base); err != nil {
		return nil, err
	}
	sort.Slice(files, func(i, j int) bool { return files[i].Path < files[j].Path })
	return files, nil
}

func requireSourcesDirectory(project string) error {
	layout, err := projectLayout(project)
	if err != nil {
		return err
	}
	directory := filepath.Join(project, layout.Sources)
	if _, err := os.Stat(directory); os.IsNotExist(err) {
		return output.Usage("project has no "+layout.Sources+"/ directory at '"+directory+"'", "use `mf project lint --fix` to create missing directories")
	} else {
		return err
	}
}

func fileKind(kind string) bool { return kind == "file" || kind == "pdf" }

func (service Service) Reconcile(project string, dryRun bool) (model.SourceIndexReport, error) {
	report := model.SourceIndexReport{Added: model.JSONArray[model.SourceIndexEntry]{}, Removed: model.JSONArray[model.SourceIndexEntry]{}, DryRun: dryRun}
	if err := requireSourcesDirectory(project); err != nil {
		return report, err
	}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	items := sourceEntries(document.Value)
	disk, err := Scan(project)
	if err != nil {
		return report, err
	}
	paths := map[string]DiskSource{}
	for _, file := range disk {
		paths[file.Path] = file
	}
	indexedPaths := map[string]bool{}
	urls := []model.Source{}
	indexed := []model.Source{}
	byName := map[string]model.Source{}
	for _, item := range items {
		byName[item.Name] = item
		if !fileKind(item.Kind) {
			urls = append(urls, item)
			continue
		}
		if item.Path == nil {
			return report, fmt.Errorf("file-type source '%s' has no path field", item.Name)
		}
		indexed = append(indexed, item)
		indexedPaths[*item.Path] = true
		if _, exists := paths[*item.Path]; !exists {
			report.Removed = append(report.Removed, model.SourceIndexEntry{Name: item.Name, Kind: item.Kind, Path: *item.Path})
		}
	}
	for _, file := range disk {
		if indexedPaths[file.Path] {
			report.KeptCount++
			continue
		}
		name, err := DeriveNameFromPath(file.Path)
		if err != nil {
			continue
		}
		report.Added = append(report.Added, model.SourceIndexEntry{Name: name, Kind: InferFileKind(file.Path), Path: file.Path})
	}
	report.KeptCount += uint64(len(urls))
	if !dryRun {
		updatedItems := append([]model.Source{}, urls...)
		for _, item := range indexed {
			updatedItems = append(updatedItems, byName[item.Name])
		}
		now := service.timestamp()
		for _, entry := range report.Added {
			path := entry.Path
			updatedItems = append(updatedItems, model.Source{Name: entry.Name, Kind: entry.Kind, SourceKind: paths[path].SourceKind, Path: &path, Tags: model.JSONArray[string]{}, AddedAt: now, UpdatedAt: now})
		}
		updated := document.Value
		updated.Sources = &updatedItems
		if _, err := service.save(project, document, updated); err != nil {
			return report, err
		}
	}
	sort.SliceStable(report.Added, func(i, j int) bool { return report.Added[i].Name < report.Added[j].Name })
	sort.SliceStable(report.Removed, func(i, j int) bool { return report.Removed[i].Name < report.Removed[j].Name })
	return report, nil
}

func (service Service) Clean(project string, dryRun bool) (model.SourceIndexReport, error) {
	report := model.SourceIndexReport{Added: model.JSONArray[model.SourceIndexEntry]{}, Removed: model.JSONArray[model.SourceIndexEntry]{}, DryRun: dryRun}
	if err := requireSourcesDirectory(project); err != nil {
		return report, err
	}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	kept := []model.Source{}
	for _, item := range sourceEntries(document.Value) {
		if !fileKind(item.Kind) {
			kept = append(kept, item)
			continue
		}
		if item.Path == nil {
			return report, fmt.Errorf("file-type source '%s' has no path field", item.Name)
		}
		if _, err := os.Stat(joinPath(project, *item.Path)); err == nil {
			kept = append(kept, item)
		} else {
			report.Removed = append(report.Removed, model.SourceIndexEntry{Name: item.Name, Kind: item.Kind, Path: *item.Path})
		}
	}
	report.KeptCount = uint64(len(kept))
	if !dryRun && len(report.Removed) > 0 {
		sort.SliceStable(kept, func(i, j int) bool { return kept[i].Name < kept[j].Name })
		updated := document.Value
		updated.Sources = &kept
		if _, err := service.save(project, document, updated); err != nil {
			return report, err
		}
	}
	sort.SliceStable(report.Removed, func(i, j int) bool { return report.Removed[i].Name < report.Removed[j].Name })
	return report, nil
}
