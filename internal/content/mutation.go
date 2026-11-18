package content

import (
	"fmt"
	"maps"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type ContentMutation struct {
	Plan   lifecycle.Plan
	Result output.VerbResult
}

type contentInventory struct {
	root          string
	before, after lifecycle.Snapshot
}

func readContentInventory(root string) (*contentInventory, error) {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return nil, err
	}
	canonical, err = filepath.Abs(canonical)
	if err != nil {
		return nil, err
	}
	before, err := lifecycle.SnapshotFiles(canonical, []string{"."})
	if err != nil {
		return nil, err
	}
	return &contentInventory{root: canonical, before: before, after: maps.Clone(before)}, nil
}

func (inventory *contentInventory) relative(absolute string) (string, error) {
	path, err := store.CanonicalWithin(inventory.root, absolute)
	if err != nil {
		return "", err
	}
	relative, err := filepath.Rel(inventory.root, path)
	if err != nil {
		return "", err
	}
	if relative == "." {
		return "", output.Usage("an entity cannot replace or remove the repository root", "")
	}
	return filepath.ToSlash(relative), nil
}

func (inventory *contentInventory) projectPath(project, relative string) (string, error) {
	if err := store.RejectTraversal(relative, "project"); err != nil {
		return "", err
	}
	target := relative
	if !filepath.IsAbs(target) {
		target = filepath.Join(project, target)
	}
	checked, err := store.CanonicalWithin(project, target)
	if err != nil {
		return "", err
	}
	if canonicalPath(project) == checked {
		return "", output.Usage("an entity cannot replace or remove the project root", "")
	}
	return inventory.relative(checked)
}

func (inventory *contentInventory) index(project string) (*store.IndexDocument, string, error) {
	name, err := inventory.relative(filepath.Join(project, "mind-index.yaml"))
	if err != nil {
		return nil, "", err
	}
	if entry, exists := inventory.before[name]; exists {
		if entry.Kind != "file" {
			return nil, "", fmt.Errorf("index is not a regular file: %s", name)
		}
		document, err := store.DecodeIndex(entry.Bytes, filepath.Join(inventory.root, name), false)
		return document, name, err
	}
	document, err := store.DecodeIndex([]byte("schema: '1'\n"), name, false)
	return document, name, err
}

func (inventory *contentInventory) setIndex(document *store.IndexDocument, name string, updated model.Index) error {
	raw, err := document.Render(updated)
	if err != nil {
		return err
	}
	entry, exists := inventory.after[name]
	if !exists {
		entry = lifecycle.Entry{Kind: "file", Mode: 0o644}
	}
	entry.Bytes = raw
	inventory.after[name] = entry
	return nil
}

func eraseContentTree(snapshot lifecycle.Snapshot, name string) {
	for path := range snapshot {
		if path == name || strings.HasPrefix(path, name+"/") {
			delete(snapshot, path)
		}
	}
}

func (inventory *contentInventory) relocate(source, target string, force bool) error {
	if source == target {
		return nil
	}
	if _, exists := inventory.after[target]; exists {
		if !force {
			return fileExists(filepath.Join(inventory.root, target))
		}
		eraseContentTree(inventory.after, target)
	}
	after, err := lifecycle.RelocateSnapshot(inventory.after, map[string]*string{source: &target})
	if err != nil {
		return err
	}
	inventory.after = after
	return nil
}

func (inventory *contentInventory) finish(operation string, result output.VerbResult) (ContentMutation, error) {
	if err := lifecycle.EnsureParents(inventory.after); err != nil {
		return ContentMutation{}, err
	}
	plan, err := lifecycle.NewPlan(inventory.root, operation, []string{"."}, inventory.before, inventory.after)
	return ContentMutation{Plan: plan, Result: result}, err
}
