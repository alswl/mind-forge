package store

import (
	"errors"
	"os"
	"path/filepath"
)

// PendingRename retains an occupied destination until its metadata commits.
// Failed rollback leaves the backup available instead of deleting evidence.
type PendingRename struct {
	old, new, directory, backup string
}

func BeginRename(old, target string) (*PendingRename, error) {
	pending := &PendingRename{old: old, new: target}
	if _, err := os.Lstat(target); err == nil {
		directory, err := os.MkdirTemp(filepath.Dir(target), ".mf-rename-*")
		if err != nil {
			return nil, err
		}
		pending.directory, pending.backup = directory, filepath.Join(directory, "replaced-file")
		if err := os.Rename(target, pending.backup); err != nil {
			return nil, errors.Join(err, os.Remove(directory))
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	if err := os.Rename(old, target); err != nil {
		if pending.backup != "" {
			if restore := os.Rename(pending.backup, target); restore != nil {
				return nil, errors.Join(err, restore)
			}
			return nil, errors.Join(err, os.Remove(pending.directory))
		}
		return nil, err
	}
	return pending, nil
}

func (pending *PendingRename) Rollback() error {
	if err := os.Rename(pending.new, pending.old); err != nil {
		return err
	}
	if pending.backup != "" {
		if err := os.Rename(pending.backup, pending.new); err != nil {
			return err
		}
		return os.RemoveAll(pending.directory)
	}
	return nil
}

func (pending *PendingRename) Commit() error {
	if pending.directory != "" {
		return os.RemoveAll(pending.directory)
	}
	return nil
}
