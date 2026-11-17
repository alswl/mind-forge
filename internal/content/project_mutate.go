package content

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type GitCommand func(context.Context, string, ...string) ([]byte, []byte, error)
type ProjectOperations struct {
	Git GitCommand
	Now time.Time
}
type ProjectRenameReport struct{ OldName, NewName, From, To string }

func runGit(ctx context.Context, root string, args ...string) ([]byte, []byte, error) {
	command := exec.CommandContext(ctx, "git", args...)
	command.Dir = root
	var stdout, stderr bytes.Buffer
	command.Stdout = &stdout
	command.Stderr = &stderr
	err := command.Run()
	return stdout.Bytes(), stderr.Bytes(), err
}

func (ops ProjectOperations) git(ctx context.Context, root string, args ...string) ([]byte, []byte, error) {
	if ops.Git != nil {
		return ops.Git(ctx, root, args...)
	}
	return runGit(ctx, root, args...)
}

func (ops ProjectOperations) stamp() string {
	now := ops.Now
	if now.IsZero() {
		now = time.Now()
	}
	return now.UTC().Format(time.RFC3339)
}

func validateProjectName(name string) error {
	hint := "use a kebab-case NAME (e.g. my-project)"
	if name == "" {
		return output.Usage("project name cannot be empty", hint)
	}
	if name == "." || name == ".." {
		return output.Usage(fmt.Sprintf("invalid project name: '%s'", name), hint)
	}
	if strings.ContainsAny(name, "/\\") {
		parts := strings.FieldsFunc(name, func(r rune) bool { return r == '/' || r == '\\' })
		bare := name
		if len(parts) > 0 {
			bare = parts[len(parts)-1]
		}
		return output.Usage(fmt.Sprintf("invalid project name '%s': path separators are not allowed", name), "use the bare name '"+bare+"'")
	}
	for _, char := range name {
		if !(char >= 'a' && char <= 'z' || char >= '0' && char <= '9' || char == '-') {
			return output.Usage(fmt.Sprintf("invalid project name '%s': only lowercase alphanumeric and hyphens allowed", name), hint)
		}
	}
	return nil
}

func legacyProjectPath(root, name string) (string, model.Manifest, error) {
	manifest, err := loadManifest(root)
	if err != nil {
		return "", manifest, err
	}
	if err := store.RejectTraversal(name, "repo"); err != nil {
		return "", manifest, err
	}
	path, err := store.CanonicalWithin(root, projectDir(root, manifest.ProjectsDir, name))
	return path, manifest, err
}

func gitFailure(err error, stderr []byte) error {
	var exit *exec.ExitError
	if errors.As(err, &exit) {
		return output.Usage("git mv failed: "+string(stderr), "check that there are no unstaged changes blocking the move")
	}
	return output.Usage("git mv failed: "+err.Error(), "ensure git is installed")
}

func (ops ProjectOperations) RenameProject(ctx context.Context, root, oldName, newName string) (ProjectRenameReport, error) {
	if err := validateProjectName(newName); err != nil {
		return ProjectRenameReport{}, err
	}
	oldPath, manifest, err := legacyProjectPath(root, oldName)
	if err != nil {
		return ProjectRenameReport{}, err
	}
	parent := projectDir(root, manifest.ProjectsDir, "")
	if _, err := os.Stat(parent); os.IsNotExist(err) {
		return ProjectRenameReport{}, output.Usage(fmt.Sprintf("projects directory '%s' not found", strings.Trim(manifest.ProjectsDir, "/")), "use `mf project list` to see available projects")
	}
	if _, err := os.Stat(oldPath); os.IsNotExist(err) {
		return ProjectRenameReport{}, output.Usage(fmt.Sprintf("project '%s' not found at %s", oldName, oldPath), "use `mf project list` to see available projects")
	}
	newPath := filepath.Join(parent, newName)
	if _, err := os.Stat(newPath); err == nil {
		return ProjectRenameReport{}, fileExists(newPath)
	}
	from, _ := filepath.Rel(root, oldPath)
	to, _ := filepath.Rel(root, newPath)
	_, _, probe := ops.git(ctx, root, "rev-parse", "--is-inside-work-tree")
	if probe != nil {
		var exit *exec.ExitError
		if !errors.As(probe, &exit) {
			return ProjectRenameReport{}, output.Usage("failed to check git repository: "+probe.Error(), "ensure git is installed")
		}
	}
	tracked := false
	if probe == nil {
		stdout, _, err := ops.git(ctx, root, "ls-files", from)
		tracked = err == nil && len(stdout) > 0
	}
	if tracked {
		_, stderr, err := ops.git(ctx, root, "mv", from, to)
		if err != nil {
			return ProjectRenameReport{}, gitFailure(err, stderr)
		}
	} else if err := os.Rename(oldPath, newPath); err != nil {
		return ProjectRenameReport{}, err
	}
	for index := range manifest.Projects {
		entry := &manifest.Projects[index]
		if entry.Name == oldName {
			entry.Name = newName
			entry.Path = store.ProjectRelativePath(manifest.ProjectsDir, newName)
			break
		}
	}
	if err := store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest); err != nil {
		return ProjectRenameReport{}, err
	}
	indexPath := filepath.Join(newPath, "mind-index.yaml")
	if _, err := os.Stat(indexPath); err == nil {
		if index, err := store.ReadIndex(indexPath, false); err == nil {
			updated := index.Value
			if updated.Articles != nil {
				for i := range *updated.Articles {
					(*updated.Articles)[i].Project = newName
					(*updated.Articles)[i].UpdatedAt = ops.stamp()
				}
			}
			_ = index.Write(indexPath, updated, false)
		}
	}
	return ProjectRenameReport{OldName: oldName, NewName: newName, From: from, To: to}, nil
}

func (ops ProjectOperations) ArchiveProject(ctx context.Context, root, name string) (model.ProjectArchiveReport, error) {
	if _, _, err := ops.git(ctx, root, "rev-parse", "--is-inside-work-tree"); err != nil {
		var exit *exec.ExitError
		if !errors.As(err, &exit) {
			return model.ProjectArchiveReport{}, output.Usage("failed to check git repository: "+err.Error(), "ensure git is installed")
		}
		return model.ProjectArchiveReport{}, output.Usage("not a git repository", "project archive requires a git repository; use 'git init' first")
	}
	from, manifest, err := legacyProjectPath(root, name)
	if err != nil {
		return model.ProjectArchiveReport{}, err
	}
	if _, err := os.Stat(from); os.IsNotExist(err) {
		return model.ProjectArchiveReport{}, output.Usage(fmt.Sprintf("project '%s' not found at %s", name, from), "use `mf project list` to see available projects")
	}
	to := filepath.Join(root, "_archived", name)
	if _, err := os.Stat(to); err == nil {
		return model.ProjectArchiveReport{}, output.Usage("_archived/"+name+" already exists", "remove or rename the existing archived project first")
	}
	if err := os.MkdirAll(filepath.Join(root, "_archived"), 0o777); err != nil {
		return model.ProjectArchiveReport{}, err
	}
	fromRel, _ := filepath.Rel(root, from)
	toRel, _ := filepath.Rel(root, to)
	if _, stderr, err := ops.git(ctx, root, "mv", fromRel, toRel); err != nil {
		return model.ProjectArchiveReport{}, gitFailure(err, stderr)
	}
	for index := range manifest.Projects {
		if manifest.Projects[index].Name == name {
			stamp := ops.stamp()
			manifest.Projects[index].ArchivedAt = &stamp
			break
		}
	}
	if err := store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest); err != nil {
		return model.ProjectArchiveReport{}, err
	}
	return model.ProjectArchiveReport{Name: name, From: fromRel, To: toRel}, nil
}

func RemoveProject(root, name string, dryRun bool) (string, error) {
	if strings.TrimSpace(name) == "" {
		return "", output.Usage("project name cannot be empty", "provide a non-empty value")
	}
	path, manifest, err := legacyProjectPath(root, name)
	if err != nil {
		return "", err
	}
	if canonicalPath(path) == canonicalPath(root) {
		return "", output.Usage("cannot remove the Mind Repo root as a project", "select a project below the repo root")
	}
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return "", output.Usage(fmt.Sprintf("project '%s' not found at %s", name, path), "use `mf project list` to see available projects")
	}
	if dryRun {
		return path, nil
	}
	if err := os.RemoveAll(path); err != nil {
		return "", err
	}
	kept := manifest.Projects[:0]
	for _, entry := range manifest.Projects {
		if entry.Name != name {
			kept = append(kept, entry)
		}
	}
	manifest.Projects = kept
	return path, store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest)
}

type ProjectImportOptions struct {
	Type, Sources, Assets *string
	Force                 bool
	Now                   time.Time
}

func ImportProject(root, directory string, options ProjectImportOptions) (model.ProjectImportReport, error) {
	canonical, err := filepath.EvalSymlinks(directory)
	if err != nil {
		return model.ProjectImportReport{}, output.Usage(fmt.Sprintf("directory '%s' not found: %s", directory, err), "provide a valid directory path to import")
	}
	canonical = canonicalPath(canonical)
	info, err := os.Stat(canonical)
	if err != nil {
		return model.ProjectImportReport{}, err
	}
	if !info.IsDir() {
		return model.ProjectImportReport{}, output.Usage("'"+directory+"' is not a directory", "")
	}
	configPath := filepath.Join(canonical, "mind.yaml")
	if _, err := os.Stat(configPath); err == nil && !options.Force {
		return model.ProjectImportReport{}, output.Usage("'"+canonical+"' already has a mind.yaml", "use --force to overwrite")
	}
	name := filepath.Base(canonical)
	if err := validateProjectName(name); err != nil {
		return model.ProjectImportReport{}, err
	}
	source, assets, kind := "sources", "assets", "arch"
	if options.Sources != nil {
		source = *options.Sources
	}
	if options.Assets != nil {
		assets = *options.Assets
	}
	if options.Type != nil {
		kind = *options.Type
	}
	resolve := func(relative string) string {
		if filepath.IsAbs(relative) {
			return relative
		}
		return filepath.Join(canonical, relative)
	}
	for _, path := range []string{resolve(source), resolve(assets)} {
		if err := os.MkdirAll(path, 0o777); err != nil {
			return model.ProjectImportReport{}, err
		}
	}
	text := fmt.Sprintf("schema: '1'\ntype: %s\nsource_dirs: [%s]\nassets_dir: %s\n", kind, source, assets)
	if err := (store.AtomicWriter{}).Write(configPath, []byte(text), 0o644); err != nil {
		return model.ProjectImportReport{}, err
	}
	count := countImportFiles(resolve(source), map[string]bool{})
	indexPath := filepath.Join(canonical, "mind-index.yaml")
	if _, err := os.Stat(indexPath); os.IsNotExist(err) || options.Force {
		stamp := (ProjectOperations{Now: options.Now}).stamp()
		articleType := kind
		if articleType != "arch" && articleType != "prd" && articleType != "blog" && articleType != "blank" {
			articleType = "blank"
		}
		articles := []model.Article{{Title: "Welcome", Project: name, Type: articleType, Status: "draft", CreatedAt: stamp, UpdatedAt: stamp}}
		index, err := store.ReadIndex(indexPath, false)
		if err != nil {
			if !options.Force {
				return model.ProjectImportReport{}, err
			}
			index, err = store.DecodeIndex([]byte("schema: '1'\n"), indexPath, false)
			if err != nil {
				return model.ProjectImportReport{}, err
			}
		}
		updated := model.DefaultIndex()
		updated.Articles = &articles
		if err := index.Write(indexPath, updated, false); err != nil {
			return model.ProjectImportReport{}, err
		}
	}
	manifest, err := loadManifest(root)
	if err != nil {
		return model.ProjectImportReport{}, err
	}
	relative, relErr := filepath.Rel(root, canonical)
	if relErr == nil && relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		path := relative
		if strings.Trim(manifest.ProjectsDir, "/") != "" && strings.Trim(manifest.ProjectsDir, "/") != "." {
			path = "./" + relative
		}
		found := false
		for _, entry := range manifest.Projects {
			if entry.Name == name {
				found = true
				break
			}
		}
		if !found {
			manifest.Projects = append(manifest.Projects, model.ProjectEntry{Name: name, Path: path, CreatedAt: (ProjectOperations{Now: options.Now}).stamp()})
		}
		if err := store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest); err != nil {
			return model.ProjectImportReport{}, err
		}
	}
	return model.ProjectImportReport{Name: name, Path: canonical, Scaffolded: true, ArticleCount: count}, nil
}

func countImportFiles(path string, active map[string]bool) uint64 {
	canonical, err := filepath.EvalSymlinks(path)
	if err != nil {
		return 0
	}
	if active[canonical] {
		return 0
	}
	info, err := os.Stat(path)
	if err != nil {
		return 0
	}
	if info.Mode().IsRegular() {
		return 1
	}
	if !info.IsDir() {
		return 0
	}
	active[canonical] = true
	defer delete(active, canonical)
	entries, err := os.ReadDir(path)
	if err != nil {
		return 0
	}
	var count uint64
	for _, entry := range entries {
		count += countImportFiles(filepath.Join(path, entry.Name()), active)
	}
	return count
}
