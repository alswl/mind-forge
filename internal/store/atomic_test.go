package store

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

var errInjected = errors.New("injected file operation failure")

type faultFilesystem struct {
	OSFilesystem
	trace       []string
	fail        string
	writeMode   os.FileMode
	shortWrite  bool
	removeError error
	closeError  error
}

func (filesystem *faultFilesystem) step(name string) error {
	filesystem.trace = append(filesystem.trace, name)
	if name == filesystem.fail {
		return errInjected
	}
	return nil
}

func (filesystem *faultFilesystem) CreateTemp(directory, pattern string) (AtomicFile, error) {
	if err := filesystem.step("create"); err != nil {
		return nil, err
	}
	file, err := filesystem.OSFilesystem.CreateTemp(directory, pattern)
	return &faultFile{AtomicFile: file, filesystem: filesystem}, err
}

func (filesystem *faultFilesystem) Rename(from, to string) error {
	if err := filesystem.step("rename"); err != nil {
		return err
	}
	return filesystem.OSFilesystem.Rename(from, to)
}

func (filesystem *faultFilesystem) Remove(path string) error {
	if filesystem.removeError != nil {
		return filesystem.removeError
	}
	return filesystem.OSFilesystem.Remove(path)
}

func (filesystem *faultFilesystem) SyncDir(directory string) error {
	if err := filesystem.step("sync-directory"); err != nil {
		return err
	}
	return filesystem.OSFilesystem.SyncDir(directory)
}

type faultFile struct {
	AtomicFile
	filesystem *faultFilesystem
}

func (file *faultFile) Write(content []byte) (int, error) {
	info, err := os.Stat(file.Name())
	if err != nil {
		return 0, err
	}
	file.filesystem.writeMode = info.Mode().Perm()
	if err := file.filesystem.step("write"); err != nil {
		return 0, err
	}
	if file.filesystem.shortWrite {
		return file.AtomicFile.Write(content[:len(content)/2])
	}
	return file.AtomicFile.Write(content)
}

func (file *faultFile) Sync() error {
	if err := file.filesystem.step("sync-file"); err != nil {
		return err
	}
	return file.AtomicFile.Sync()
}

func (file *faultFile) Chmod(mode os.FileMode) error {
	if err := file.filesystem.step("chmod"); err != nil {
		return err
	}
	return file.AtomicFile.Chmod(mode)
}

func (file *faultFile) Chown(uid, gid int) error {
	if err := file.filesystem.step("chown"); err != nil {
		return err
	}
	return file.AtomicFile.Chown(uid, gid)
}

func (file *faultFile) Close() error {
	err := errors.Join(file.AtomicFile.Close(), file.filesystem.closeError)
	if injected := file.filesystem.step("close"); injected != nil {
		return injected
	}
	return err
}

func TestAtomicWritePreservesModeAndOrdersDurability(t *testing.T) {
	path := filepath.Join(t.TempDir(), "index.yaml")
	if err := os.WriteFile(path, []byte("before"), 0o600); err != nil {
		t.Fatal(err)
	}
	beforeInfo, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	beforeUID, beforeGID, err := fileOwner(beforeInfo)
	if err != nil {
		t.Fatal(err)
	}
	filesystem := &faultFilesystem{}
	if err := (AtomicWriter{Filesystem: filesystem}).Write(path, []byte("after"), 0o644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("existing mode changed: %v (%v)", info, err)
	}
	uid, gid, err := fileOwner(info)
	if err != nil || uid != beforeUID || gid != beforeGID {
		t.Fatalf("file ownership changed: %d:%d to %d:%d (%v)", beforeUID, beforeGID, uid, gid, err)
	}
	if trace := strings.Join(filesystem.trace, ","); trace != "create,write,chown,chmod,sync-file,close,rename,sync-directory" {
		t.Fatalf("durability order: %s", trace)
	}
}

func TestAtomicStagingRemainsPrivateWhileWriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "value")
	filesystem := &faultFilesystem{}
	if err := (AtomicWriter{Filesystem: filesystem}).Write(path, []byte("payload"), 0o644); err != nil {
		t.Fatal(err)
	}
	if filesystem.writeMode != 0o600 {
		t.Fatalf("staging mode during payload write: %o, want 600", filesystem.writeMode)
	}
}

func TestAtomicWriteFailuresRetainThePublishedState(t *testing.T) {
	for _, stage := range []string{"create", "chown", "chmod", "write", "sync-file", "close", "rename", "sync-directory"} {
		t.Run(stage, func(t *testing.T) {
			directory := t.TempDir()
			path := filepath.Join(directory, "value")
			if err := os.WriteFile(path, []byte("before"), 0o640); err != nil {
				t.Fatal(err)
			}
			filesystem := &faultFilesystem{fail: stage}
			if err := (AtomicWriter{Filesystem: filesystem}).Write(path, []byte("after"), 0o644); !errors.Is(err, errInjected) {
				t.Fatalf("failure not returned: %v", err)
			}
			t.Logf("failure=%s trace=%v", stage, filesystem.trace)
			want := "before"
			if stage == "sync-directory" {
				want = "after"
			}
			actual, err := os.ReadFile(path)
			if err != nil || string(actual) != want {
				t.Fatalf("published bytes=%q, want %q (%v)", actual, want, err)
			}
			entries, err := os.ReadDir(directory)
			if err != nil || len(entries) != 1 {
				t.Fatalf("staging file leaked: %v (%v)", entries, err)
			}
		})
	}
}

func TestAtomicShortWriteRetainsDestinationAndReportsCleanupFailures(t *testing.T) {
	for _, cleanupFails := range []bool{false, true} {
		t.Run(fmt.Sprint(cleanupFails), func(t *testing.T) {
			directory := t.TempDir()
			path := filepath.Join(directory, "value")
			if err := os.WriteFile(path, []byte("before"), 0o640); err != nil {
				t.Fatal(err)
			}
			closeError := errors.New("close failed")
			removeError := errors.New("remove failed")
			fs := &faultFilesystem{shortWrite: true}
			if cleanupFails {
				fs.closeError, fs.removeError = closeError, removeError
			}
			err := (AtomicWriter{Filesystem: fs}).Write(path, []byte("replacement"), 0o600)
			t.Logf("short-write cleanup-fails=%t trace=%v error=%v", cleanupFails, fs.trace, err)
			if !errors.Is(err, io.ErrShortWrite) {
				t.Fatalf("short write lost: %v", err)
			}
			if cleanupFails && (!errors.Is(err, closeError) || !errors.Is(err, removeError)) {
				t.Fatalf("cleanup errors lost: %v", err)
			}
			raw, readError := os.ReadFile(path)
			if readError != nil || string(raw) != "before" {
				t.Fatalf("destination changed: %q (%v)", raw, readError)
			}
			info, statError := os.Stat(path)
			if statError != nil || info.Mode().Perm() != 0o640 {
				t.Fatalf("destination mode changed: %v (%v)", info, statError)
			}
			for _, operation := range fs.trace {
				if operation == "rename" {
					t.Fatal("short payload was published")
				}
			}
			entries, readError := os.ReadDir(directory)
			want := 1
			if cleanupFails {
				want = 2
			}
			if readError != nil || len(entries) != want {
				t.Fatalf("staging cleanup: %v (%v)", entries, readError)
			}
		})
	}
}
