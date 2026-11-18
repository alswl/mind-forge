package content

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type RepoInitReport struct {
	Path               string                  `json:"path"`
	Created            bool                    `json:"created"`
	AlreadyExisted     bool                    `json:"already_existed"`
	CreatedFiles       model.JSONArray[string] `json:"created_files"`
	CreatedDirectories model.JSONArray[string] `json:"created_directories"`
	Skipped            model.JSONArray[string] `json:"skipped"`
}

type RepoTargetKind int

const (
	ExistingEmptyDirectory RepoTargetKind = iota
	NewDirectory
	ExistingRepository
	MalformedManifest
	InvalidFileTarget
	InvalidPath
	UnsafeNonEmptyDirectory
)

func ClassifyRepoTarget(target string) (RepoTargetKind, error) {
	clean := filepath.Clean(target)
	last := ""
	for _, component := range strings.Split(target, string(filepath.Separator)) {
		if component != "" && component != "." {
			last = component
		}
	}
	if target == "" || last == ".." || filepath.Base(clean) == string(filepath.Separator) {
		return InvalidPath, nil
	}
	info, err := os.Stat(target)
	if os.IsNotExist(err) || errors.Is(err, syscall.ENOTDIR) {
		parent := filepath.Dir(clean)
		info, err := os.Stat(parent)
		if os.IsNotExist(err) {
			return InvalidPath, output.Usage(fmt.Sprintf("parent directory '%s' does not exist", parent), "create the parent directory first, or choose a different path")
		}
		if err != nil {
			return InvalidPath, err
		}
		if !info.IsDir() {
			return InvalidPath, output.Usage(fmt.Sprintf("parent path '%s' is not a directory", parent), "")
		}
		return NewDirectory, nil
	}
	if err != nil {
		return InvalidPath, err
	}
	if info.Mode().IsRegular() {
		return InvalidFileTarget, nil
	}
	if !info.IsDir() {
		return InvalidPath, nil
	}
	if _, err := os.Stat(filepath.Join(target, "minds.yaml")); err == nil {
		if _, err := store.ReadManifest(filepath.Join(target, "minds.yaml")); err != nil {
			var failure *output.Error
			if errors.As(err, &failure) && (failure.Kind == "parse_error" || failure.Kind == "incompatible_schema") {
				return MalformedManifest, nil
			}
			return InvalidPath, err
		}
		return ExistingRepository, nil
	} else if !os.IsNotExist(err) {
		return InvalidPath, err
	}
	entries, err := os.ReadDir(target)
	if err != nil {
		return InvalidPath, err
	}
	if len(entries) == 0 {
		return ExistingEmptyDirectory, nil
	}
	return UnsafeNonEmptyDirectory, nil
}

func InitializeRepository(target string) (RepoInitReport, error) {
	kind, err := ClassifyRepoTarget(target)
	if err != nil {
		return RepoInitReport{}, err
	}
	if kind == NewDirectory || kind == ExistingEmptyDirectory {
		parent, err := filepath.Abs(filepath.Dir(target))
		if err != nil {
			return RepoInitReport{}, err
		}
		if root := app.DetectRoot(parent, 50); root != "" {
			return RepoInitReport{}, output.Usage(fmt.Sprintf("cannot initialize '%s' inside an existing Mind Repo at '%s'", target, root), "use `mf project new` to create a project inside a Mind Repo")
		}
	}
	report := RepoInitReport{Path: target, CreatedFiles: model.JSONArray[string]{}, CreatedDirectories: model.JSONArray[string]{}, Skipped: model.JSONArray[string]{}}
	if kind == ExistingRepository {
		if path, err := filepath.EvalSymlinks(target); err == nil {
			if absolute, err := filepath.Abs(path); err == nil {
				report.Path = absolute
			}
		}
		report.AlreadyExisted = true
		report.Skipped = append(report.Skipped, "minds.yaml")
		return report, nil
	}
	switch kind {
	case MalformedManifest:
		return report, output.Usage(fmt.Sprintf("cannot initialize '%s': existing minds.yaml is malformed or incompatible", target), "fix or remove the existing minds.yaml first, then try again")
	case InvalidFileTarget:
		return report, output.Usage(fmt.Sprintf("cannot initialize '%s': path is a file, not a directory", target), "choose a directory path instead")
	case InvalidPath:
		return report, output.Usage(fmt.Sprintf("invalid target path '%s'", target), "use a valid directory name without path traversal")
	case UnsafeNonEmptyDirectory:
		return report, output.Usage(fmt.Sprintf("cannot initialize '%s': directory is not empty", target), "choose an empty directory, a new path, or run from an existing Mind Repo")
	}
	if kind == NewDirectory {
		if err := os.Mkdir(target, 0o777); err != nil {
			return report, output.Usage(fmt.Sprintf("cannot create directory '%s': %s", target, err), "check parent directory permissions")
		}
	}
	manifest := model.DefaultManifest()
	if err := store.WriteManifest(filepath.Join(target, "minds.yaml"), manifest); err != nil {
		if kind == NewDirectory {
			_ = os.Remove(target)
		}
		return report, err
	}
	if err := os.Mkdir(filepath.Join(target, manifest.ProjectsDir), 0o777); err != nil {
		if kind == NewDirectory {
			_ = os.Remove(target)
		}
		return report, err
	}
	if path, err := filepath.EvalSymlinks(target); err == nil {
		if absolute, err := filepath.Abs(path); err == nil {
			report.Path = absolute
		}
	}
	report.Created = true
	report.CreatedFiles = append(report.CreatedFiles, "minds.yaml")
	report.CreatedDirectories = append(report.CreatedDirectories, manifest.ProjectsDir)
	return report, nil
}
