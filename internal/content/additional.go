package content

import (
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/textcase"
)

type Additional struct {
	Kind            string `json:"kind"`
	Identity        string `json:"identity"`
	ProjectIdentity string `json:"project_identity"`
	MindIdentity    string `json:"mind_identity"`
	Mind            string `json:"mind"`
	AdditionalKind  string `json:"additional_kind"`
	Path            string `json:"path"`
	Shape           string `json:"shape"`
}

func SelectAdditional(items []Additional, selector string) (Additional, error) {
	if strings.Contains(selector, "/") && !strings.Contains(selector, "::") {
		unsafe := strings.HasPrefix(selector, "/") || strings.HasPrefix(selector, "./")
		for _, part := range strings.Split(selector, "/") {
			unsafe = unsafe || part == ".."
		}
		if unsafe {
			return Additional{}, &output.Error{Kind: "unsafe_path", Message: selector, Lifecycle: true}
		}
	}
	for _, item := range items {
		if item.Identity == selector || item.Path == selector {
			return item, nil
		}
	}
	candidates := []string{}
	var selected Additional
	for _, item := range items {
		if item.AdditionalKind == selector || item.Mind+"::additional::"+item.AdditionalKind == selector {
			candidates = append(candidates, item.Identity)
			selected = item
		}
	}
	switch len(candidates) {
	case 0:
		return Additional{}, &output.Error{Kind: "not_found", Message: "additional not found: " + selector}
	case 1:
		return selected, nil
	default:
		encoded, _ := output.CompactJSON(candidates)
		hint := "select an exact output identity or configure default_output"
		return Additional{}, &output.Error{Kind: "selection_ambiguous", Message: "ambiguous selector " + selector + ": " + string(encoded), Hint: &hint, Candidates: &candidates}
	}
}

// AdditionalRenameTarget retains ownership and shape in both physical layouts.
func AdditionalRenameTarget(root, project string, item Additional, target string, items []Additional) (string, error) {
	if _, err := CheckedPath(project, target); err != nil {
		return "", err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return "", err
	}
	invalid := func() (string, error) {
		return "", &output.Error{Kind: "invalid_additional_target", Message: "target must be a concrete additional path with the same mind and shape", Lifecycle: true}
	}
	parts := strings.Split(target, "/")
	if len(parts) != 2 || config.ContentLayout == nil {
		return invalid()
	}
	file := item.Shape == "file"
	kind := parts[0]
	if *config.ContentLayout == "category" {
		name := item.Mind
		if file {
			name += ".md"
		}
		if parts[1] != name {
			return invalid()
		}
	} else {
		if parts[0] != item.Mind || file && !strings.HasSuffix(parts[1], ".md") {
			return invalid()
		}
		kind = parts[1]
		if file {
			kind = strings.TrimSuffix(kind, ".md")
		}
		copy := *config
		copy.ReservedRoots = nil
		config = &copy
	}
	if !lifecycle.ValidAdditionalKind(kind) || kind == "prompt" {
		return invalid()
	}
	for name := range reserved(config) {
		if textcase.ASCIIEqualFold(kind, name) {
			return invalid()
		}
	}
	children, err := ChildProjectRoots(root, project)
	if err != nil {
		return "", err
	}
	if children[kind] {
		return invalid()
	}
	conflicts := []string{}
	for _, other := range items {
		if other.Mind == item.Mind && textcase.ASCIIEqualFold(other.AdditionalKind, kind) {
			conflicts = append(conflicts, other.Path)
		}
	}
	if len(conflicts) > 0 {
		return "", &output.Error{Kind: "path_conflict", Message: target, Candidates: &conflicts, Lifecycle: true}
	}
	entries, err := os.ReadDir(project)
	if err != nil {
		return "", err
	}
	for _, entry := range entries {
		if textcase.ASCIIEqualFold(entry.Name(), parts[0]) && (entry.Name() != parts[0] || !entry.IsDir()) {
			conflicts := []string{entry.Name()}
			return "", &output.Error{Kind: "path_conflict", Message: target, Candidates: &conflicts, Lifecycle: true}
		}
	}
	if _, err := os.Stat(filepath.Join(project, target)); err == nil {
		return "", fileExists(filepath.Join(project, target))
	} else if !os.IsNotExist(err) {
		return "", err
	}
	return kind, nil
}

func DiscoverAdditional(root, project string) ([]Additional, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	minds, err := Discover(root, project)
	if err != nil {
		return nil, err
	}
	children, err := ChildProjectRoots(root, project)
	if err != nil {
		return nil, err
	}
	ignored := map[string]bool{}
	for _, name := range []string{"build", "docs", "templates", "cache", "target", "node_modules", "vendor", "dist", "_archived"} {
		ignored[name] = true
	}
	if config.Layout != nil {
		for _, value := range []*string{config.Layout.Templates, config.Layout.BuildOutput} {
			if value != nil {
				first, _, _ := strings.Cut(*value, "/")
				ignored[first] = true
			}
		}
	}
	reserved := map[string]bool{}
	for _, name := range config.ReservedRoots {
		reserved[name] = true
	}
	bundle := config.ContentLayout != nil && *config.ContentLayout == "bundle"
	files := lifecycle.Snapshot{}
	err = filepath.WalkDir(project, func(path string, entry fs.DirEntry, walkError error) error {
		if walkError != nil {
			return &output.Error{Kind: "inventory_failed", Message: walkError.Error(), Lifecycle: true, Cause: walkError}
		}
		relative, err := filepath.Rel(project, path)
		if err != nil {
			return err
		}
		if relative == "." {
			return nil
		}
		name := entry.Name()
		relative = filepath.ToSlash(relative)
		depth := strings.Count(relative, "/") + 1
		core := name == "outputs" || name == "prompts" || name == "thinking" || name == "sources" || name == "assets"
		skip := strings.HasPrefix(name, ".") || depth == 1 && (children[name] || ignored[name] || reserved[name] && !core) || bundle && depth == 2 && ignored[name]
		if skip {
			if entry.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		if !utf8.ValidString(relative) {
			return &output.Error{Kind: "ownership_unresolved", Message: "non UTF-8 path", Lifecycle: true}
		}
		kind := "file"
		if entry.IsDir() {
			kind = "directory"
		} else if !entry.Type().IsRegular() {
			return &output.Error{Kind: "unsafe_path", Message: "non-regular content path: " + relative, Lifecycle: true}
		}
		files[relative] = lifecycle.Entry{Kind: kind}
		return nil
	})
	if err != nil {
		return nil, err
	}
	childNames := make([]string, 0, len(children))
	for name := range children {
		childNames = append(childNames, name)
	}
	sort.Strings(childNames)
	owned, err := lifecycle.CollectOwnedRoots(*config, minds, childNames, files)
	if err != nil {
		return nil, err
	}
	projectIdentity, err := ProjectIdentityPath(root, project)
	if err != nil {
		return nil, err
	}
	result := make([]Additional, 0)
	for _, item := range owned {
		if item.Kind == "prompt" || item.Kind == "thinking" || item.Kind == "outputs" || item.Kind == "sources" || item.Kind == "assets" {
			continue
		}
		mindIdentity := projectIdentity + "::" + item.Mind
		shape := "directory"
		if item.File {
			shape = "file"
		}
		result = append(result, Additional{Kind: "additional", Identity: mindIdentity + "::additional::" + item.Kind, ProjectIdentity: projectIdentity, MindIdentity: mindIdentity, Mind: item.Mind, AdditionalKind: item.Kind, Path: item.Path, Shape: shape})
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Identity < result[j].Identity })
	return result, nil
}
