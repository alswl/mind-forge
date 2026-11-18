//go:build darwin || linux

package store

import (
	"os"
	"sync"

	"github.com/alswl/mind-forge/internal/output"
	"golang.org/x/sys/unix"
)

type DirectoryLock struct {
	file *os.File
	once sync.Once
	err  error
}

func AcquireWriteLock(root string) (*DirectoryLock, error) {
	if err := CheckLocalFilesystem(root); err != nil {
		return nil, err
	}
	file, err := os.Open(root)
	if err != nil {
		return nil, err
	}
	if info, err := file.Stat(); err != nil || !info.IsDir() {
		file.Close()
		if err != nil {
			return nil, err
		}
		return nil, &output.Error{Kind: "unsafe_path", Message: "write lock requires a directory", Lifecycle: true}
	}
	if err := unix.Flock(int(file.Fd()), unix.LOCK_EX|unix.LOCK_NB); err != nil {
		file.Close()
		return nil, &output.Error{Kind: "write_locked", Message: err.Error(), Lifecycle: true, Cause: err}
	}
	return &DirectoryLock{file: file}, nil
}

func (lock *DirectoryLock) Close() error {
	lock.once.Do(func() {
		unlockError := unix.Flock(int(lock.file.Fd()), unix.LOCK_UN)
		closeError := lock.file.Close()
		lock.err = unlockError
		if lock.err == nil {
			lock.err = closeError
		}
	})
	return lock.err
}
