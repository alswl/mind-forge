package store

import (
	"bytes"
	"os"
)

// CommitIndex reports publication independently from a writer error, because
// callers must not roll back content whose index has already reached disk.
func CommitIndex(path string, render func() ([]byte, error), write func(string, []byte) error) (bool, error) {
	raw, err := render()
	if err != nil {
		return false, err
	}
	if write != nil {
		err = write(path, raw)
	} else {
		err = (AtomicWriter{}).Write(path, raw, 0644)
	}
	if err == nil {
		return true, nil
	}
	actual, readError := os.ReadFile(path)
	return readError == nil && bytes.Equal(actual, raw), err
}
