package config

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func projectDirectory(root, projectsDir, name string) string {
	directory := strings.Trim(projectsDir, "/")
	if directory == "" || directory == "." {
		return filepath.Join(root, name)
	}
	return filepath.Join(root, directory, name)
}

func manifestProject(root string, manifest model.Manifest, name string) string {
	for _, project := range manifest.Projects {
		stripped := strings.TrimPrefix(project.Path, "./")
		if project.Name == name || filepath.Base(stripped) == name || stripped == name {
			if filepath.IsAbs(stripped) {
				return stripped
			}
			return filepath.Join(root, stripped)
		}
	}
	return ""
}

func repoRelative(root, path string) string {
	relative, err := filepath.Rel(root, path)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return path
	}
	if relative == "." {
		return ""
	}
	return relative
}

// ResolveProject permits an absent project only for commands that create one.
// Merely selecting a destination never changes the filesystem.
func ResolveProject(root string, selector *string, cwd string, allowMissing bool) (string, error) {
	manifest := model.DefaultManifest()
	if path := filepath.Join(root, "minds.yaml"); exists(path) {
		loaded, err := store.ReadManifest(path)
		if err != nil {
			return "", err
		}
		manifest = loaded
	}
	if selector == nil {
		current, boundary := canonical(cwd), canonical(root)
		for {
			if exists(filepath.Join(current, "mind.yaml")) {
				if path := manifestProject(root, manifest, repoRelative(boundary, current)); path != "" {
					return path, nil
				}
				return current, nil
			}
			parent := filepath.Dir(current)
			if current == boundary || parent == current {
				break
			}
			current = parent
		}
		return "", output.Usage("could not detect current project; run from a project directory or specify --project", "use `mf project list` to see available projects; a path argument must lie inside a project")
	}
	input := *selector
	var candidate string
	if input == "." || input == ".." || strings.ContainsAny(input, "/\\") {
		if err := store.RejectTraversal(input, "repo"); err != nil {
			return "", err
		}
		target := input
		if !filepath.IsAbs(input) {
			target = filepath.Join(canonical(cwd), input)
		}
		path, err := store.CanonicalWithin(root, target)
		if err != nil {
			return "", err
		}
		candidate = path
	} else {
		registered := manifestProject(root, manifest, input)
		local := filepath.Join(cwd, input)
		if !exists(filepath.Join(local, "mind.yaml")) {
			local = ""
		}
		switch {
		case registered != "" && local != "" && canonical(registered) != canonical(local):
			return "", output.Usage(fmt.Sprintf("project selector '%s' is ambiguous: it matches both '%s' and '%s'", input, repoRelative(canonical(root), canonical(registered)), repoRelative(canonical(root), canonical(local))), "use a path (e.g. './name' for the cwd-relative one, or the full project path) to disambiguate")
		case registered != "":
			candidate = registered
		case local != "":
			candidate = local
		default:
			candidate = projectDirectory(root, manifest.ProjectsDir, input)
		}
	}
	if !allowMissing && !exists(filepath.Join(candidate, "mind.yaml")) {
		return "", output.Usage(fmt.Sprintf("project '%s' not found (resolved to '%s')", input, repoRelative(canonical(root), canonical(candidate))), "use `mf project list` to see available projects")
	}
	return candidate, nil
}
