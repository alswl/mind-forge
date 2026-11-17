package store

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// ReplaceDirectory stages a complete flat directory before publishing it.
// Recovery for multi-entity operations belongs to the lifecycle engine.
func ReplaceDirectory(path string, files map[string][]byte) error {
	return writeDirectory(path, files, true)
}

// CreateDirectory publishes a staged flat directory at an absent destination.
func CreateDirectory(path string, files map[string][]byte) error {
	return writeDirectory(path, files, false)
}

func writeDirectory(path string, files map[string][]byte, replace bool) (writeError error) {
	info, err := os.Lstat(path)
	if replace {
		if err != nil {
			return err
		}
		if !info.IsDir() {
			return fmt.Errorf("not a directory: %s", path)
		}
	} else if err == nil {
		return fmt.Errorf("directory destination already exists: %s", path)
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	parent := filepath.Dir(path)
	staging, err := os.MkdirTemp(parent, "."+filepath.Base(path)+".rewrite-*")
	if err != nil {
		return err
	}
	defer func() { writeError = errors.Join(writeError, os.RemoveAll(staging)) }()
	names := make([]string, 0, len(files))
	for name := range files {
		if name == "" || name == "." || name == ".." || strings.ContainsAny(name, "/\\") {
			return fmt.Errorf("invalid directory entry: %q", name)
		}
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if err := (AtomicWriter{}).Write(filepath.Join(staging, name), files[name], 0o644); err != nil {
			return err
		}
	}
	mode := os.FileMode(0o755)
	if info != nil {
		mode = info.Mode().Perm()
	}
	if err := os.Chmod(staging, mode); err != nil {
		return err
	}
	if err := (OSFilesystem{}).SyncDir(staging); err != nil {
		return err
	}
	if !replace {
		if _, err := os.Lstat(path); err == nil {
			return fmt.Errorf("directory destination already exists: %s", path)
		} else if !errors.Is(err, os.ErrNotExist) {
			return err
		}
		if err := os.Rename(staging, path); err != nil {
			return err
		}
		return (OSFilesystem{}).SyncDir(parent)
	}
	backup, err := os.MkdirTemp(parent, "."+filepath.Base(path)+".backup-*")
	if err != nil {
		return err
	}
	if err := os.Remove(backup); err != nil {
		return err
	}
	if err := os.Rename(path, backup); err != nil {
		return err
	}
	if err := os.Rename(staging, path); err != nil {
		return errors.Join(err, os.Rename(backup, path))
	}
	// Publication is complete even if synchronization or cleanup fails.
	if err := (OSFilesystem{}).SyncDir(parent); err != nil {
		return err
	}
	if err := os.RemoveAll(backup); err != nil {
		return err
	}
	return (OSFilesystem{}).SyncDir(parent)
}
