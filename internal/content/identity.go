package content

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type Identity struct{ Project, Mind, Output string }

type EntityKind string

const (
	Article EntityKind = "article"
	Asset   EntityKind = "asset"
	Source  EntityKind = "source"
)

type LocalIdentity struct {
	Kind                                EntityKind
	ProjectIdentity, Path, ResolvedPath string
}

func ResolvePathSelector(input, cwd string, boundary *string, hint string) (string, error) {
	if strings.TrimSpace(input) == "" {
		return "", output.Usage("path cannot be empty", "provide a path")
	}
	if err := store.RejectTraversal(input, hint); err != nil {
		return "", err
	}
	if canonical, err := filepath.EvalSymlinks(cwd); err == nil {
		cwd = canonical
	}
	target := input
	if !filepath.IsAbs(target) {
		target = filepath.Join(cwd, target)
	}
	if boundary != nil {
		return store.CanonicalWithin(*boundary, target)
	}
	return target, nil
}

func ValidateEntityPath(project, input string) error {
	if err := store.RejectTraversal(input, "project"); err != nil {
		return err
	}
	if strings.ContainsAny(input, "/\\") {
		target := input
		if !filepath.IsAbs(target) {
			target = filepath.Join(project, target)
		}
		_, err := store.CanonicalWithin(project, target)
		return err
	}
	return nil
}

func NormalizeEntitySelector(project, projectIdentity string, kind EntityKind, input string) (LocalIdentity, error) {
	if err := store.RejectTraversal(input, "project"); err != nil {
		return LocalIdentity{}, err
	}
	directory := map[EntityKind]string{Article: "docs", Asset: "assets", Source: "sources"}[kind]
	if directory == "" {
		return LocalIdentity{}, output.Usage("unknown path entity kind: "+string(kind), "")
	}
	relative := input
	if !strings.ContainsAny(input, "/\\") {
		relative = directory + "/" + input
	}
	target := relative
	if !filepath.IsAbs(target) {
		target = filepath.Join(project, target)
	}
	path, err := store.CanonicalWithin(project, target)
	if err != nil {
		return LocalIdentity{}, err
	}
	return LocalIdentity{Kind: kind, ProjectIdentity: projectIdentity, Path: relative, ResolvedPath: path}, nil
}

func (identity Identity) String() string {
	text := identity.Project + "::" + identity.Mind
	if identity.Output != "" {
		text += "::" + identity.Output
	}
	return text
}

func ParseIdentity(text string) (Identity, error) {
	parts := strings.Split(text, "::")
	if len(parts) < 2 || len(parts) > 3 {
		return Identity{}, output.Usage("invalid content identity: "+text, "")
	}
	if parts[0] != "" {
		for _, part := range strings.Split(parts[0], "/") {
			if part == "" || part == "." || part == ".." || strings.ContainsAny(part, "\\") {
				return Identity{}, output.Usage("invalid project identity: "+parts[0], "")
			}
		}
	}
	if err := ValidateName(parts[1]); err != nil {
		return Identity{}, err
	}
	identity := Identity{Project: parts[0], Mind: parts[1]}
	if len(parts) == 3 {
		if err := ValidateName(parts[2]); err != nil {
			return Identity{}, err
		}
		identity.Output = parts[2]
	}
	return identity, nil
}

func ValidateName(name string) error {
	if name == "" || strings.HasPrefix(name, ".") || strings.HasSuffix(name, ".md") || strings.ContainsAny(name, "/\\:") || strings.TrimSpace(name) != name || strings.ContainsFunc(name, unicode.IsControl) {
		return output.Usage(fmt.Sprintf("invalid mind/output name: %q", name), "")
	}
	return nil
}

func CheckedPath(project, relative string) (string, error) {
	path := project
	for index, part := range strings.Split(filepath.ToSlash(relative), "/") {
		// Rust path components ignore empty/interior-dot components, while a
		// leading dot and traversal are not normal components.
		if part == "" {
			if index == 0 && relative != "" {
				return "", output.Usage("unsafe content path: "+relative, "")
			}
			continue
		}
		if part == ".." || part == "." && index == 0 {
			return "", output.Usage("unsafe content path: "+relative, "")
		}
		if part == "." {
			continue
		}
		path = filepath.Join(path, part)
		info, err := os.Lstat(path)
		if err != nil && !os.IsNotExist(err) {
			return "", err
		}
		if err == nil && info.Mode()&os.ModeSymlink != 0 {
			return "", output.Usage("symlink content path refused: "+path, "")
		}
	}
	return path, nil
}

func ProjectFor(root, cwd string, selected, selector *string) (string, error) {
	if selector != nil {
		if prefix, _, ok := strings.Cut(*selector, "::"); ok {
			path, err := CheckedPath(root, prefix)
			if err != nil {
				return "", err
			}
			if info, err := os.Stat(filepath.Join(path, "mind.yaml")); err == nil && !info.IsDir() {
				if selected != nil {
					explicit, err := app.ResolveProject(root, selected, cwd, false)
					if err != nil {
						return "", err
					}
					a, _ := filepath.EvalSymlinks(explicit)
					b, _ := filepath.EvalSymlinks(path)
					if a != b {
						return "", output.Usage("identity conflicts with --project", "")
					}
				}
				return path, nil
			}
		}
	}
	return app.ResolveProject(root, selected, cwd, false)
}

func ResolveMind(root, project, selector string) (model.Mind, error) {
	minds, err := Discover(root, project)
	if err != nil {
		return model.Mind{}, err
	}
	for _, mind := range minds {
		if mind.Identity == selector || mind.Name == selector {
			return mind, nil
		}
	}
	return model.Mind{}, &output.Error{Kind: "not_found", Message: "mind not found: " + selector}
}

func ResolveOutput(root, project, selector string) (model.Output, error) {
	minds, err := Discover(root, project)
	if err != nil {
		return model.Output{}, err
	}
	var outputs []model.Output
	for _, mind := range minds {
		outputs = append(outputs, mind.Outputs...)
	}
	for _, item := range outputs {
		if item.Identity == selector || item.Path == selector {
			return item, nil
		}
	}
	for _, mind := range minds {
		if mind.Identity != selector && mind.Name != selector {
			continue
		}
		if mind.Name == selector {
			var collisions []model.Output
			for _, item := range outputs {
				if item.Name == selector && item.Mind != mind.Name {
					collisions = append(collisions, item)
				}
			}
			if len(collisions) > 0 {
				candidates := []string{mind.Identity}
				for _, item := range append(collisions, mind.Outputs...) {
					candidates = append(candidates, item.Identity)
				}
				return model.Output{}, ambiguous(selector, candidates)
			}
		}
		if mind.DefaultOutput != nil {
			for _, item := range mind.Outputs {
				if item.Name == *mind.DefaultOutput {
					return item, nil
				}
			}
			return model.Output{}, &output.Error{Kind: "not_found", Message: "default output missing: " + mind.Identity + "::" + *mind.DefaultOutput}
		}
		return uniqueOutput(mind.Outputs, selector)
	}
	var matches []model.Output
	for _, item := range outputs {
		if item.Name == selector || item.Mind+"::"+item.Name == selector {
			matches = append(matches, item)
		}
	}
	return uniqueOutput(matches, selector)
}

func uniqueOutput(outputs []model.Output, selector string) (model.Output, error) {
	if len(outputs) == 1 {
		return outputs[0], nil
	}
	if len(outputs) == 0 {
		return model.Output{}, &output.Error{Kind: "not_found", Message: "no output for: " + selector}
	}
	candidates := make([]string, 0, len(outputs))
	for _, item := range outputs {
		candidates = append(candidates, item.Identity)
	}
	return model.Output{}, ambiguous(selector, candidates)
}

func ambiguous(selector string, candidates []string) error {
	quoted := make([]string, len(candidates))
	for index, name := range candidates {
		quoted[index] = fmt.Sprintf("%q", name)
	}
	hint := "select an exact output identity or configure default_output"
	return &output.Error{Kind: "selection_ambiguous", Message: fmt.Sprintf("ambiguous selector %s: [%s]", selector, strings.Join(quoted, ", ")), Candidates: &candidates, Hint: &hint}
}
