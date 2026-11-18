package asset

import (
	"errors"
	"os"
	"path/filepath"
	"syscall"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type MoveOutcome struct {
	Name, OldPath, NewPath string
	DryRun                 bool
}

func notFound(selector string) error {
	hint := "use `mf asset list` to see available assets"
	return &output.Error{Kind: "not_found", Message: "asset '" + selector + "' not found", Hint: &hint}
}
func (service Service) Move(project, target, selector string, dryRun bool) (MoveOutcome, error) {
	result := MoveOutcome{DryRun: dryRun}
	if project == target {
		return result, output.Usage("asset source and destination projects must differ", "")
	}
	document, err := readIndex(project)
	if err != nil {
		return result, err
	}
	items := assetEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Name == selector || item.Path == selector {
			pos = i
			break
		}
	}
	if pos < 0 {
		return result, notFound(selector)
	}
	entry := items[pos]
	filename := assetFileName(entry.Path)
	if filename == "" {
		return result, output.Usage("asset path has no filename", "")
	}
	layout, err := projectLayout(target)
	if err != nil {
		return result, err
	}
	newPath := layout.Assets + "/" + filename
	destination, err := readIndex(target)
	if err != nil {
		return result, err
	}
	targetItems := assetEntries(destination.Value)
	for _, item := range targetItems {
		if item.Name == entry.Name || item.Path == newPath {
			return result, output.Usage("destination already contains asset '"+entry.Name+"'", "choose a different asset name or remove the destination entry")
		}
	}
	result.Name, result.OldPath, result.NewPath = entry.Name, entry.Path, newPath
	if dryRun {
		return result, nil
	}
	oldFull, newFull := joinPath(project, entry.Path), joinPath(target, newPath)
	if _, err := os.Stat(oldFull); os.IsNotExist(err) {
		return result, &output.Error{Kind: "not_found", Message: "asset file '" + oldFull + "' not found"}
	}
	if _, err := os.Stat(newFull); err == nil {
		return result, output.Usage("destination asset path '"+newPath+"' already exists", "remove the destination path before moving")
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
	updated.Assets = &items
	sourcePublished, sourceError := service.save(project, document, updated)
	if sourceError != nil && !sourcePublished {
		return result, errors.Join(sourceError, pending.Rollback())
	}
	entry.Path = newPath
	targetItems = append(targetItems, entry)
	targetIndex := destination.Value
	targetIndex.Assets = &targetItems
	targetPublished, targetError := service.save(target, destination, targetIndex)
	if targetError != nil && !targetPublished {
		_, restoreError := service.save(project, document, document.Value)
		return result, errors.Join(sourceError, targetError, pending.Rollback(), restoreError)
	}
	return result, errors.Join(sourceError, targetError, pending.Commit())
}

func (service Service) Rename(project, oldPath, newPath string, force bool) error {
	if oldPath == "" || newPath == "" {
		return output.Usage("asset paths must not be empty", "")
	}
	document, err := readIndex(project)
	if err != nil {
		return err
	}
	items := assetEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Path == oldPath || item.Name == oldPath {
			pos = i
			break
		}
	}
	if pos < 0 {
		hint := "use `mf asset list` to see available assets"
		return &output.Error{Kind: "not_found", Message: "asset at '" + oldPath + "' not found", Hint: &hint}
	}
	for _, item := range items {
		if oldPath != newPath && item.Path == newPath && !force {
			return output.Usage("an asset at '"+newPath+"' already exists", "use --force to overwrite")
		}
	}
	oldFull, newFull := joinPath(project, oldPath), joinPath(project, newPath)
	var pending *store.PendingRename
	if _, err := os.Stat(oldFull); err == nil {
		if err := os.MkdirAll(filepath.Dir(newFull), 0755); err != nil {
			return err
		}
		if _, err := os.Stat(newFull); err == nil && !force {
			return fileExists(newFull)
		}
		if oldFull != newFull {
			pending, err = store.BeginRename(oldFull, newFull)
			if err != nil {
				return err
			}
		}
	}
	items[pos].Name, items[pos].Path = RenameName(newPath), newPath
	updated := document.Value
	updated.Assets = &items
	published, err := service.save(project, document, updated)
	if pending != nil {
		if err != nil && !published {
			return errors.Join(err, pending.Rollback())
		}
		err = errors.Join(err, pending.Commit())
	}
	return err
}

func (service Service) Remove(project, selector string, force, dryRun bool) (model.AssetRemoveReport, error) {
	report := model.AssetRemoveReport{Force: force, DryRun: dryRun, References: model.JSONArray[model.Reference]{}, SideEffects: model.JSONArray[model.PlannedChange]{}}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	items := assetEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Path == selector || item.Name == selector {
			pos = i
			break
		}
	}
	if pos < 0 {
		return report, output.Usage("asset '"+selector+"' not found", "use `mf asset list -p <project>` to see available assets")
	}
	item := items[pos]
	report.Removed = joinPath(project, item.Path)
	report.WasReferenced, report.References = References(project, document.Value, item.Name)
	if report.WasReferenced && !force {
		return report, output.Usage("asset '"+item.Name+"' is referenced by articles; use --force to remove anyway", "check which articles reference this asset before removal")
	}
	exists := false
	if info, err := os.Stat(report.Removed); err == nil {
		exists = true
		if info.IsDir() && !dryRun {
			return report, &os.PathError{Op: "remove", Path: report.Removed, Err: syscall.EISDIR}
		}
		report.SideEffects = append(report.SideEffects, model.PlannedChange{Op: "remove_file", Path: report.Removed})
	}
	indexPath := filepath.Join(project, "mind-index.yaml")
	report.SideEffects = append(report.SideEffects, model.PlannedChange{Op: "update_yaml", Path: indexPath, Old: &item.Name}, model.PlannedChange{Op: "refresh_index", Path: indexPath})
	if dryRun {
		return report, nil
	}
	directory := ""
	var pending *store.PendingRename
	if exists {
		directory, err = os.MkdirTemp(filepath.Dir(report.Removed), ".mf-asset-remove-*")
		if err != nil {
			return report, err
		}
		pending, err = store.BeginRename(report.Removed, filepath.Join(directory, "removed"))
		if err != nil {
			_ = os.RemoveAll(directory)
			return report, err
		}
	}
	for i, candidate := range items {
		if candidate.Name == item.Name {
			pos = i
			break
		}
	}
	items = append(items[:pos], items[pos+1:]...)
	updated := document.Value
	updated.Assets = &items
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
