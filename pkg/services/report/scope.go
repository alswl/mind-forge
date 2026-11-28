package report

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/config"
	dal "github.com/alswl/mind-forge/pkg/dal/report"
)

const repoMarker = "minds.yaml"
const projectMarker = "mind.yaml"

type Scope struct {
	Directory string
	Reports   string
	boundary  string
}

func RegistryRoot(path string) (string, error) {
	if path == "" {
		return "", nil
	}
	dir, err := canonicalDir(path)
	if err != nil {
		return "", fmt.Errorf("registry root: %w", err)
	}
	if err := checkMarker(dir, repoMarker); err != nil {
		return "", fmt.Errorf("registry root: %w", err)
	}
	return dir, nil
}

func ProjectScope(repo, cwd string, selector *string) (Scope, error) {
	path, err := config.ResolveProject(repo, selector, cwd, false)
	if err != nil {
		return Scope{}, err
	}
	project, err := filepath.Abs(path)
	if err != nil {
		return Scope{}, err
	}
	rel, err := filepath.Rel(repo, project)
	if err != nil {
		return Scope{}, err
	}
	if err := dal.Local(rel); err != nil {
		return Scope{}, fmt.Errorf("%w: project %q is outside the registry", ErrInvalid, project)
	}
	scope := Scope{Directory: project, boundary: repo}
	if err := scope.Ready(); err != nil {
		return Scope{}, err
	}
	settings, err := config.EffectiveConfig(project, repo)
	if err != nil {
		return Scope{}, err
	}
	layout, _ := config.ResolveEffectiveLayout(&settings)
	if err := dal.Local(layout.Templates); err != nil {
		return Scope{}, fmt.Errorf("%w: layout.templates: %w", ErrInvalid, err)
	}
	scope.Reports = filepath.Join(layout.Templates, "reports")
	for _, category := range []struct{ name, path string }{
		{"layout.articles", layout.Articles}, {"layout.sources", layout.Sources},
		{"layout.assets", layout.Assets}, {"layout.build_output", layout.BuildOutput},
	} {
		base, err := resolveExistingPrefix(filepath.Join(project, category.path))
		if err != nil {
			return Scope{}, fmt.Errorf("%w: %s: %w", ErrInvalid, category.name, err)
		}
		if pathsOverlap(filepath.Join(project, scope.Reports), base) {
			return Scope{}, fmt.Errorf("%w: %s overlaps report directory %q", ErrInvalid, category.name, scope.Reports)
		}
	}
	return scope, nil
}

func StandaloneScope(directory string) (Scope, error) {
	dir, err := canonicalDir(directory)
	if err != nil {
		return Scope{}, err
	}
	return Scope{Directory: dir}, nil
}

func canonicalDir(directory string) (string, error) {
	path, err := filepath.Abs(directory)
	if err != nil {
		return "", err
	}
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		return "", err
	}
	root, err := os.OpenRoot(resolved)
	if err != nil {
		return "", err
	}
	return resolved, root.Close()
}

func resolveExistingPrefix(path string) (string, error) {
	resolved, err := filepath.EvalSymlinks(path)
	if !errors.Is(err, fs.ErrNotExist) || filepath.Dir(path) == path {
		return resolved, err
	}
	parent, err := resolveExistingPrefix(filepath.Dir(path))
	if err != nil {
		return "", err
	}
	return filepath.Join(parent, filepath.Base(path)), nil
}

func pathsOverlap(a, b string) bool {
	a, b = strings.ToLower(filepath.Clean(a)), strings.ToLower(filepath.Clean(b))
	return a == b || strings.HasPrefix(a, b+"/") || strings.HasPrefix(b, a+"/")
}

func checkMarker(directory, marker string) error {
	root, err := os.OpenRoot(directory)
	if err != nil {
		return err
	}
	defer root.Close()
	return dal.Regular(root, marker)
}

func (scope Scope) Ready() error {
	dir, err := scope.open()
	if err != nil {
		return err
	}
	return dir.Close()
}

func (scope Scope) open() (*os.Root, error) {
	if scope.Directory == "" {
		return nil, fmt.Errorf("%w: report directory is required", ErrInvalid)
	}
	boundary := scope.boundary
	if boundary == "" {
		boundary = filepath.Dir(scope.Directory)
	}
	parent, err := os.OpenRoot(boundary)
	if err != nil {
		return nil, err
	}
	defer parent.Close()
	rel, err := filepath.Rel(boundary, scope.Directory)
	if err != nil {
		return nil, err
	}
	if scope.boundary != "" {
		if err := dal.Regular(parent, repoMarker); err != nil {
			return nil, err
		}
		if err := dal.Regular(parent, filepath.Join(rel, projectMarker)); err != nil {
			return nil, err
		}
	} else if err := dal.Check(parent, rel, false); err != nil {
		return nil, err
	}
	return parent.OpenRoot(rel)
}

func (scope Scope) openReport(slug string) (*os.Root, error) {
	if slug != "" && !ValidSlug(slug) {
		return nil, fmt.Errorf("%w: invalid report slug %q", ErrInvalid, slug)
	}
	root, err := scope.open()
	if err != nil {
		return nil, fmt.Errorf("%w: project: %w", ErrInvalid, err)
	}
	if scope.Reports == "" {
		if slug == "" || slug == filepath.Base(scope.Directory) {
			return root, nil
		}
		root.Close()
		return nil, fmt.Errorf("%w: report %q", ErrNotFound, slug)
	}
	defer root.Close()
	if slug == "" {
		return nil, fmt.Errorf("%w: report slug is required", ErrInvalid)
	}
	dir := filepath.Join(scope.Reports, slug)
	if err := dal.Check(root, dir, false); err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, fmt.Errorf("%w: report %q", ErrNotFound, slug)
		}
		return nil, fmt.Errorf("%w: %s: %w", ErrInvalid, dir, err)
	}
	report, err := root.OpenRoot(dir)
	if err != nil {
		return nil, fmt.Errorf("%w: %s: %w", ErrInvalid, dir, err)
	}
	return report, nil
}

func (service Service) Ready(scope Scope) error {
	if err := scope.Ready(); err != nil {
		return err
	}
	catalog, err := service.discover()
	if err != nil {
		return err
	}
	for _, id := range []string{"builtin/brief", "builtin/comparison", "builtin/trend"} {
		item, err := catalog.lookup(Renderer, id)
		if err != nil {
			return err
		}
		if _, err := compileSchema(item.Schema); err != nil {
			return err
		}
	}
	return nil
}

func (service Service) Scaffold(ctx context.Context, scope Scope, slug, renderer string) (ScaffoldResult, error) {
	if scope.Reports == "" || !ValidSlug(slug) {
		return ScaffoldResult{}, fmt.Errorf("%w: expected a project and valid report slug, got %q", ErrInvalid, slug)
	}
	if renderer == "" {
		renderer = "builtin/brief"
	}
	catalog, err := service.discover()
	if err != nil {
		return ScaffoldResult{}, err
	}
	item, err := catalog.lookup(Renderer, renderer)
	if err != nil {
		return ScaffoldResult{}, err
	}
	root, err := scope.open()
	if err != nil {
		return ScaffoldResult{}, fmt.Errorf("%w: project: %w", ErrInvalid, err)
	}
	defer root.Close()
	dir := filepath.Join(scope.Reports, slug)
	files, err := starter(item)
	if err != nil {
		return ScaffoldResult{}, err
	}
	if err := dal.Scaffold(ctx, root, dir, files); err != nil {
		if errors.Is(err, fs.ErrExist) {
			return ScaffoldResult{}, fmt.Errorf("%w: %s", ErrConflict, dir)
		}
		return ScaffoldResult{}, fmt.Errorf("scaffold %s: %w", dir, err)
	}
	return ScaffoldResult{Directory: dir, Renderer: renderer}, nil
}
