package lifecycle

import (
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/textcase"
)

type OwnedRoot struct {
	Mind, Kind, Path string
	File             bool
}

func OwnedDestination(layout, owner, kind string, file bool) string {
	if layout == "category" {
		if kind == "prompt" {
			return "prompts/" + owner + ".md"
		}
		if kind == "thinking" {
			return "thinking/" + owner + ".md"
		}
		if file {
			return kind + "/" + owner + ".md"
		}
		return kind + "/" + owner
	}
	if file {
		return owner + "/" + kind + ".md"
	}
	return owner + "/" + kind
}

func validKind(kind string) bool {
	if kind == "" {
		return false
	}
	for _, part := range strings.Split(kind, "-") {
		if part == "" {
			return false
		}
		for _, char := range part {
			if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9') {
				return false
			}
		}
	}
	return true
}

func ValidAdditionalKind(kind string) bool { return validKind(kind) }

func hiddenPath(path string) bool {
	for _, part := range strings.Split(path, "/") {
		if strings.HasPrefix(part, ".") {
			return true
		}
	}
	return false
}

func ownershipConflict(kind, message string, candidates []string) error {
	return &output.Error{Kind: kind, Message: message, Candidates: &candidates, Lifecycle: true}
}

func asciiEqualFold(left, right string) bool { return textcase.ASCIIEqualFold(left, right) }

func validateRole(kind string, file bool, path string) error {
	for _, core := range []string{"prompt", "thinking", "outputs", "sources", "assets"} {
		if kind != core && strings.EqualFold(kind, core) {
			return ownershipConflict("path_conflict", "core role case collision", []string{path, core})
		}
	}
	if (kind == "prompt" || kind == "thinking") && !file || (kind == "outputs" || kind == "sources" || kind == "assets") && file {
		return rejected("ownership_unresolved", path+": invalid core role shape")
	}
	return nil
}

// CollectOwnedRoots receives discovery facts from the content resolver. Titles
// and import provenance never establish ownership, and zero-output minds remain
// known owners even when there is no output directory on disk.
func CollectOwnedRoots(config model.MindConfig, minds []model.Mind, childRoots []string, files Snapshot) ([]OwnedRoot, error) {
	if config.ContentLayout == nil {
		return nil, rejected("ownership_unresolved", "legacy project has no explicit layout")
	}
	layout := *config.ContentLayout
	if layout != "category" && layout != "bundle" {
		return nil, rejected("invalid_metadata", "unknown content layout")
	}
	names := map[string]bool{}
	identities := make([]string, 0, len(minds))
	for _, mind := range minds {
		names[mind.Name] = true
		identities = append(identities, mind.Identity)
	}
	excluded := map[string]bool{}
	for _, name := range []string{"build", "docs", "templates", "cache", "target", "node_modules", "vendor", "dist", "_archived"} {
		excluded[name] = true
	}
	for _, name := range childRoots {
		excluded[name] = true
	}
	if layout == "category" {
		for _, name := range config.ReservedRoots {
			excluded[name] = true
		}
	}
	if config.Layout != nil {
		for _, pair := range []struct {
			configured *string
			standard   string
		}{{config.Layout.Articles, "docs"}, {config.Layout.Sources, "sources"}, {config.Layout.Assets, "assets"}} {
			if pair.configured == nil || *pair.configured == pair.standard {
				continue
			}
			for name, entry := range files {
				if (name == *pair.configured || strings.HasPrefix(name, *pair.configured+"/")) && entry.Kind == "file" && !hiddenPath(name) {
					return nil, rejected("ownership_unresolved", "configured legacy core root "+*pair.configured+": explicit ownership mapping required")
				}
			}
		}
		for _, configured := range []*string{config.Layout.Articles, config.Layout.Sources, config.Layout.Assets, config.Layout.Templates, config.Layout.BuildOutput} {
			if configured != nil {
				excluded[strings.Split(*configured, "/")[0]] = true
			}
		}
	}
	for _, role := range []string{"outputs", "prompts", "prompt", "thinking", "sources", "assets"} {
		delete(excluded, role)
	}
	found := []OwnedRoot{}
	for _, path := range snapshotNames(files) {
		if hiddenPath(path) {
			continue
		}
		parts := strings.Split(path, "/")
		if len(parts) != 2 {
			continue
		}
		file := files[path].Kind == "file"
		var owner, kind string
		if layout == "category" {
			if excluded[parts[0]] {
				continue
			}
			kind = parts[0]
			if kind == "prompts" {
				kind = "prompt"
			}
			owner = parts[1]
			if file {
				owner = strings.TrimSuffix(owner, ".md")
			}
			if !names[owner] {
				return nil, ownershipConflict("ownership_unresolved", path+": no unique known mind owner", identities)
			}
			if !validKind(kind) || file && !strings.HasSuffix(parts[1], ".md") {
				return nil, rejected("ownership_unresolved", path+": unsupported role/additional shape")
			}
		} else {
			if !names[parts[0]] {
				continue
			}
			owner = parts[0]
			kind = parts[1]
			if file {
				if !strings.HasSuffix(kind, ".md") {
					return nil, rejected("ownership_unresolved", path+": unknown top-level file")
				}
				kind = strings.TrimSuffix(kind, ".md")
			}
			if excluded[kind] {
				continue
			}
			if !validKind(kind) {
				return nil, rejected("ownership_unresolved", path+": unsupported additional kind")
			}
		}
		if err := validateRole(kind, file, path); err != nil {
			return nil, err
		}
		found = append(found, OwnedRoot{Mind: owner, Kind: kind, Path: path, File: file})
	}
	kinds := map[string]string{}
	keys := map[[2]string]bool{}
	for _, item := range found {
		kind := legacyLower(item.Kind)
		if previous, exists := kinds[kind]; layout == "category" && exists && previous != item.Kind {
			candidates := []string{}
			for _, other := range found {
				if strings.EqualFold(other.Kind, item.Kind) {
					candidates = append(candidates, other.Path)
				}
			}
			return nil, ownershipConflict("path_conflict", "category case collision", candidates)
		}
		kinds[kind] = item.Kind
		key := [2]string{legacyLower(item.Mind), kind}
		if keys[key] {
			candidates := []string{}
			for _, other := range found {
				if strings.EqualFold(other.Kind, item.Kind) && asciiEqualFold(other.Mind, item.Mind) {
					candidates = append(candidates, other.Path)
				}
			}
			return nil, ownershipConflict("path_conflict", "file/directory or case collision", candidates)
		}
		keys[key] = true
	}
	for name, entry := range files {
		if strings.HasPrefix(name, "docs/") && entry.Kind == "file" && !hiddenPath(name) {
			return nil, rejected("ownership_unresolved", "legacy docs contain files; explicit ownership mapping required")
		}
	}
	return found, nil
}

func OwnedMapping(items []OwnedRoot, layout string, names map[string]string) map[string]string {
	mapping := map[string]string{}
	for _, item := range items {
		if owner, exists := names[item.Mind]; exists {
			mapping[item.Path] = OwnedDestination(layout, owner, item.Kind, item.File)
		}
	}
	return mapping
}
