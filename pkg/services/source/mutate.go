package source

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/services/content"
)

type RenameOutcome struct {
	Before, After model.Source
	DryRun        bool
}
type MoveOutcome struct {
	Name    string `json:"name"`
	OldPath string `json:"old_path"`
	NewPath string `json:"new_path"`
	DryRun  bool   `json:"dry_run"`
}

func (service Service) Rename(project, cwd, name, target string, force, dryRun bool) (RenameOutcome, error) {
	result := RenameOutcome{DryRun: dryRun}
	if strings.TrimSpace(name) == "" {
		return result, output.Usage("old source name must not be empty", "")
	}
	if strings.TrimSpace(target) == "" {
		return result, output.Usage("rename target must not be empty", "")
	}
	document, err := readIndex(project)
	if err != nil {
		return result, err
	}
	items := sourceEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Name == name {
			pos = i
			break
		}
	}
	if pos < 0 {
		return result, notFound(name)
	}
	resolved, err := content.ResolvePathSelector(target, cwd, &project, "project")
	if err != nil {
		return result, err
	}
	newName, err := DeriveNameFromPath(resolved)
	if err != nil {
		return result, output.Usage("target '"+target+"' has no valid file name", "provide a path ending in a file name")
	}
	if name != newName && !force {
		for _, item := range items {
			if item.Name == newName {
				return result, output.Usage("a source named '"+newName+"' already exists", "use --force to overwrite")
			}
		}
	}
	result.Before = items[pos]
	result.After = items[pos]
	result.After.Name = newName
	if result.Before.Path != nil {
		rel, err := relativePath(project, resolved)
		if err != nil {
			return result, err
		}
		result.After.Path = &rel
	}
	oldFull := ""
	if result.Before.Path != nil {
		oldFull = joinPath(project, *result.Before.Path)
		if _, err := os.Stat(oldFull); err == nil {
			parent := filepath.Dir(resolved)
			if _, err := os.Stat(parent); os.IsNotExist(err) {
				rel, _ := relativePath(project, parent)
				return result, &output.Error{Kind: "not_found", Message: "target directory does not exist: '" + rel + "'", Hint: stringPointer("create the directory first, or choose an existing one")}
			}
			if _, err := os.Stat(resolved); err == nil && resolved != oldFull && !force {
				return result, fileExists(resolved)
			}
		}
	}
	if dryRun {
		return result, nil
	}
	var pending *store.PendingRename
	if oldFull != "" && oldFull != resolved {
		if _, err := os.Stat(oldFull); err == nil {
			pending, err = store.BeginRename(oldFull, resolved)
			if err != nil {
				return result, err
			}
		}
	}
	items[pos] = result.After
	updated := document.Value
	updated.Sources = &items
	published, err := service.save(project, document, updated)
	if pending != nil {
		if err != nil && !published {
			return result, errors.Join(err, pending.Rollback())
		}
		err = errors.Join(err, pending.Commit())
	}
	return result, err
}

func stringPointer(value string) *string { return &value }

func (service Service) Move(project, target, selector string, dryRun bool) (MoveOutcome, error) {
	result := MoveOutcome{DryRun: dryRun}
	if project == target {
		return result, output.Usage("source and destination projects must differ", "")
	}
	document, err := readIndex(project)
	if err != nil {
		return result, err
	}
	items := sourceEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Name == selector || item.Path != nil && *item.Path == selector {
			pos = i
			break
		}
	}
	if pos < 0 {
		return result, notFound(selector)
	}
	entry := items[pos]
	if entry.Path == nil {
		return result, output.Usage("only local sources can be moved", "")
	}
	filename := fileName(*entry.Path)
	if filename == "" {
		return result, output.Usage("source path has no filename", "")
	}
	newPath := filepath.ToSlash(filepath.Join("sources", entry.Kind, filename))
	destination, err := readIndex(target)
	if err != nil {
		return result, err
	}
	targetItems := sourceEntries(destination.Value)
	for _, item := range targetItems {
		if item.Name == entry.Name || item.Path != nil && *item.Path == newPath {
			return result, output.Usage("destination already contains source '"+entry.Name+"'", "choose a different source name or remove the destination entry")
		}
	}
	result.Name, result.OldPath, result.NewPath = entry.Name, *entry.Path, newPath
	if dryRun {
		return result, nil
	}
	oldFull, newFull := joinPath(project, *entry.Path), joinPath(target, newPath)
	if _, err := os.Stat(oldFull); os.IsNotExist(err) {
		return result, &output.Error{Kind: "not_found", Message: "source file '" + oldFull + "' not found"}
	}
	if _, err := os.Stat(newFull); err == nil {
		return result, output.Usage("destination source path '"+newPath+"' already exists", "remove the destination path before moving")
	}
	if err := os.MkdirAll(filepath.Dir(newFull), 0755); err != nil {
		return result, err
	}
	pending, err := store.BeginRename(oldFull, newFull)
	if err != nil {
		return result, err
	}
	items = append(items[:pos], items[pos+1:]...)
	updated := document.Value
	updated.Sources = &items
	sourcePublished, sourceError := service.save(project, document, updated)
	if sourceError != nil && !sourcePublished {
		return result, errors.Join(sourceError, pending.Rollback())
	}
	entry.Path = &newPath
	targetItems = append(targetItems, entry)
	targetIndex := destination.Value
	targetIndex.Sources = &targetItems
	targetPublished, targetError := service.save(target, destination, targetIndex)
	if targetError != nil && !targetPublished {
		_, restoreError := service.save(project, document, document.Value)
		return result, errors.Join(sourceError, targetError, pending.Rollback(), restoreError)
	}
	return result, errors.Join(sourceError, targetError, pending.Commit())
}

func (service Service) Remove(project, selector string, keepFile, force, dryRun bool) (model.SourceRemoveReport, error) {
	report := model.SourceRemoveReport{Force: force, DryRun: dryRun, References: model.JSONArray[model.Reference]{}, SideEffects: model.JSONArray[model.PlannedChange]{}}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	items := sourceEntries(document.Value)
	name, err := ResolveSelector(items, selector)
	if err != nil {
		return report, err
	}
	pos := -1
	for i, item := range items {
		if item.Name == name {
			pos = i
			break
		}
	}
	if pos < 0 {
		return report, notFound(selector)
	}
	report.Source = items[pos]
	if document.Value.Articles != nil {
		for _, article := range *document.Value.Articles {
			path := article.Path
			report.References = append(report.References, model.Reference{FromKind: "article", FromID: article.Title, FromPath: &path, Kind: "index"})
		}
	}
	if len(report.References) > 0 && !force {
		names := []string{}
		for _, ref := range report.References {
			names = append(names, ref.FromID)
		}
		return report, output.Usage("source '"+name+"' is referenced by: "+strings.Join(names, ", ")+". Use --force to remove anyway.", "check which articles reference this source before removal")
	}
	file := ""
	if !keepFile && fileKind(report.Source.Kind) && report.Source.Path != nil {
		candidate := joinPath(project, *report.Source.Path)
		if info, err := os.Stat(candidate); err == nil {
			if info.IsDir() {
				return report, &os.PathError{Op: "remove", Path: candidate, Err: syscall.EISDIR}
			}
			file = candidate
			report.FileDeleted = true
			report.SideEffects = append(report.SideEffects, model.PlannedChange{Op: "remove_file", Path: candidate})
		} else if !os.IsNotExist(err) {
			return report, err
		}
	}
	indexPath := filepath.Join(project, "mind-index.yaml")
	report.SideEffects = append(report.SideEffects, model.PlannedChange{Op: "update_yaml", Path: indexPath, Old: &name}, model.PlannedChange{Op: "refresh_index", Path: indexPath})
	if dryRun {
		return report, nil
	}
	var pending *store.PendingRename
	directory := ""
	if file != "" {
		directory, err = os.MkdirTemp(filepath.Dir(file), ".mf-source-remove-*")
		if err != nil {
			return report, err
		}
		pending, err = store.BeginRename(file, filepath.Join(directory, "removed"))
		if err != nil {
			_ = os.RemoveAll(directory)
			return report, err
		}
	}
	items = append(items[:pos], items[pos+1:]...)
	updated := document.Value
	updated.Sources = &items
	published, err := service.save(project, document, updated)
	if pending != nil && err != nil && !published {
		if restore := pending.Rollback(); restore != nil {
			return report, errors.Join(err, restore)
		}
	}
	if directory != "" {
		err = errors.Join(err, os.RemoveAll(directory))
	}
	return report, err
}
