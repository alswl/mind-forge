package store

import (
	"errors"
	"os"
	"path/filepath"
	"syscall"
)

// PublishFile stages bytes or a symlink and keeps the previous file until metadata commits.
func PublishFile(destination string, raw []byte, mode os.FileMode, link string, commit func() (bool, error)) (result error) {
	if info, err := os.Lstat(destination); err == nil && info.IsDir() {
		return &os.PathError{Op: "remove", Path: destination, Err: syscall.EISDIR}
	}
	directory, err := os.MkdirTemp(filepath.Dir(destination), ".mf-publish-*")
	if err != nil {
		return err
	}
	retain := false
	defer func() {
		if !retain {
			result = errors.Join(result, os.RemoveAll(directory))
		}
	}()
	staged := filepath.Join(directory, "content")
	if link != "" {
		err = os.Symlink(link, staged)
	} else {
		err = (AtomicWriter{}).Write(staged, raw, mode)
	}
	if err != nil {
		return err
	}
	pending, err := BeginRename(staged, destination)
	if err != nil {
		return err
	}
	published, err := commit()
	if err != nil && !published {
		if rollback := pending.Rollback(); rollback != nil {
			retain = true
			return errors.Join(err, rollback)
		}
		return err
	}
	return errors.Join(err, pending.Commit())
}
