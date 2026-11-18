package store

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

type AtomicFile interface {
	io.Writer
	Sync() error
	Close() error
	Chmod(os.FileMode) error
	Chown(int, int) error
	Name() string
}

type AtomicFilesystem interface {
	Lstat(string) (os.FileInfo, error)
	CreateTemp(string, string) (AtomicFile, error)
	Rename(string, string) error
	Remove(string) error
	SyncDir(string) error
}

type OSFilesystem struct{}

func (OSFilesystem) Lstat(path string) (os.FileInfo, error) { return os.Lstat(path) }
func (OSFilesystem) Rename(from, to string) error           { return os.Rename(from, to) }
func (OSFilesystem) Remove(path string) error               { return os.Remove(path) }
func (OSFilesystem) CreateTemp(directory, pattern string) (AtomicFile, error) {
	return os.CreateTemp(directory, pattern)
}

func (OSFilesystem) SyncDir(path string) error {
	directory, err := os.Open(path)
	if err != nil {
		return err
	}
	defer directory.Close()
	return directory.Sync()
}

type AtomicWriter struct {
	Filesystem AtomicFilesystem
}

// Write reports a post-rename sync error without pretending that publication
// was undone. The lifecycle component owns recovery across multiple writes.
func (writer AtomicWriter) Write(path string, content []byte, mode os.FileMode) (writeError error) {
	filesystem := writer.Filesystem
	if filesystem == nil {
		filesystem = OSFilesystem{}
	}
	info, err := filesystem.Lstat(path)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if info != nil {
		if !info.Mode().IsRegular() {
			return fmt.Errorf("refusing to replace non-regular file: %s", path)
		}
		mode = info.Mode() & (os.ModePerm | os.ModeSetuid | os.ModeSetgid | os.ModeSticky)
	}
	file, err := filesystem.CreateTemp(filepath.Dir(path), "."+filepath.Base(path)+".tmp-*")
	if err != nil {
		return err
	}
	closed := false
	defer func() {
		if !closed {
			if err := file.Close(); err != nil {
				writeError = errors.Join(writeError, err)
			}
		}
		if err := filesystem.Remove(file.Name()); err != nil && !errors.Is(err, os.ErrNotExist) {
			writeError = errors.Join(writeError, fmt.Errorf("remove staging file: %w", err))
		}
	}()
	count, err := file.Write(content)
	if err != nil {
		return err
	}
	if count != len(content) {
		return io.ErrShortWrite
	}
	if info != nil {
		uid, gid, err := fileOwner(info)
		if err != nil {
			return err
		}
		if err := file.Chown(uid, gid); err != nil {
			return err
		}
	}
	// Writing and chown can clear special mode bits. Apply the final mode only
	// after both, keeping the staging file private while its payload is written.
	if err := file.Chmod(mode); err != nil {
		return err
	}
	if err := file.Sync(); err != nil {
		return err
	}
	closed = true
	if err := file.Close(); err != nil {
		return err
	}
	if err := filesystem.Rename(file.Name(), path); err != nil {
		return err
	}
	return filesystem.SyncDir(filepath.Dir(path))
}
