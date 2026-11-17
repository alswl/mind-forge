package lifecycle

import (
	"io"
	"os"

	"github.com/alswl/mind-forge/internal/store"
)

type DurableFile interface {
	io.Writer
	Sync() error
	Close() error
	Chmod(os.FileMode) error
}

type Filesystem interface {
	Lstat(string) (os.FileInfo, error)
	ReadFile(string) ([]byte, error)
	ReadDir(string) ([]os.DirEntry, error)
	OpenFile(string, int, os.FileMode) (DurableFile, error)
	Mkdir(string, os.FileMode) error
	MkdirAll(string, os.FileMode) error
	Chmod(string, os.FileMode) error
	Rename(string, string) error
	Remove(string) error
	SyncDir(string) error
}

type OSFilesystem struct{}

func (OSFilesystem) Lstat(path string) (os.FileInfo, error)     { return os.Lstat(path) }
func (OSFilesystem) ReadFile(path string) ([]byte, error)       { return os.ReadFile(path) }
func (OSFilesystem) ReadDir(path string) ([]os.DirEntry, error) { return os.ReadDir(path) }
func (OSFilesystem) OpenFile(path string, flags int, mode os.FileMode) (DurableFile, error) {
	return os.OpenFile(path, flags, mode)
}
func (OSFilesystem) Mkdir(path string, mode os.FileMode) error    { return os.Mkdir(path, mode) }
func (OSFilesystem) MkdirAll(path string, mode os.FileMode) error { return os.MkdirAll(path, mode) }
func (OSFilesystem) Chmod(path string, mode os.FileMode) error    { return os.Chmod(path, mode) }
func (OSFilesystem) Rename(from, to string) error                 { return os.Rename(from, to) }
func (OSFilesystem) Remove(path string) error                     { return os.Remove(path) }
func (OSFilesystem) SyncDir(path string) error                    { return (store.OSFilesystem{}).SyncDir(path) }

type Engine struct {
	Filesystem  Filesystem
	Inventory   func(string, []string) (Snapshot, error)
	AcquireLock func(string) (io.Closer, error)
	Checkpoint  func(string) error
}

func (engine Engine) defaults() Engine {
	if engine.Filesystem == nil {
		engine.Filesystem = OSFilesystem{}
	}
	if engine.Inventory == nil {
		engine.Inventory = SnapshotFiles
	}
	if engine.AcquireLock == nil {
		engine.AcquireLock = func(root string) (io.Closer, error) { return store.AcquireWriteLock(root) }
	}
	return engine
}

func (engine Engine) checkpoint(stage string) error {
	if engine.Checkpoint != nil {
		return engine.Checkpoint(stage)
	}
	return nil
}

func unixMode(bits uint32) os.FileMode {
	mode := os.FileMode(bits & 0o777)
	if bits&0o4000 != 0 {
		mode |= os.ModeSetuid
	}
	if bits&0o2000 != 0 {
		mode |= os.ModeSetgid
	}
	if bits&0o1000 != 0 {
		mode |= os.ModeSticky
	}
	return mode
}
