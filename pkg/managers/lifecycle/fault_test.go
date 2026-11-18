package lifecycle

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type faultFilesystem struct {
	OSFilesystem
	root         string
	trace        []string
	fail         string
	ordinal      int
	seen         map[string]int
	partialWrite bool
}

func (fs *faultFilesystem) relative(path string) string {
	name, err := filepath.Rel(fs.root, path)
	if err != nil {
		return path
	}
	return filepath.ToSlash(name)
}

func (fs *faultFilesystem) step(name string) error {
	fs.trace = append(fs.trace, name)
	if fs.seen == nil {
		fs.seen = map[string]int{}
	}
	fs.seen[name]++
	ordinal := fs.ordinal
	if ordinal == 0 {
		ordinal = 1
	}
	if name == fs.fail && fs.seen[name] == ordinal {
		return errComponentFault
	}
	return nil
}

func (fs *faultFilesystem) OpenFile(path string, flags int, mode os.FileMode) (DurableFile, error) {
	if err := fs.step("open:" + fs.relative(path)); err != nil {
		return nil, err
	}
	file, err := fs.OSFilesystem.OpenFile(path, flags, mode)
	if err != nil {
		return nil, err
	}
	return &faultFile{DurableFile: file, filesystem: fs, path: fs.relative(path)}, nil
}
func (fs *faultFilesystem) Mkdir(path string, mode os.FileMode) error {
	if err := fs.step("mkdir:" + fs.relative(path)); err != nil {
		return err
	}
	return fs.OSFilesystem.Mkdir(path, mode)
}
func (fs *faultFilesystem) MkdirAll(path string, mode os.FileMode) error {
	if err := fs.step("mkdir-all:" + fs.relative(path)); err != nil {
		return err
	}
	return fs.OSFilesystem.MkdirAll(path, mode)
}
func (fs *faultFilesystem) Chmod(path string, mode os.FileMode) error {
	if err := fs.step("chmod:" + fs.relative(path)); err != nil {
		return err
	}
	return fs.OSFilesystem.Chmod(path, mode)
}
func (fs *faultFilesystem) Rename(from, to string) error {
	if err := fs.step("rename:" + fs.relative(from) + "->" + fs.relative(to)); err != nil {
		return err
	}
	return fs.OSFilesystem.Rename(from, to)
}
func (fs *faultFilesystem) Remove(path string) error {
	if err := fs.step("remove:" + fs.relative(path)); err != nil {
		return err
	}
	return fs.OSFilesystem.Remove(path)
}
func (fs *faultFilesystem) SyncDir(path string) error {
	if err := fs.step("sync-dir:" + fs.relative(path)); err != nil {
		return err
	}
	return fs.OSFilesystem.SyncDir(path)
}

type faultFile struct {
	DurableFile
	filesystem *faultFilesystem
	path       string
}

func (file *faultFile) Write(raw []byte) (int, error) {
	if err := file.filesystem.step("write:" + file.path); err != nil {
		if file.filesystem.partialWrite {
			n, writeError := file.DurableFile.Write(raw[:len(raw)/2])
			return n, errors.Join(err, writeError)
		}
		return 0, err
	}
	return file.DurableFile.Write(raw)
}
func (file *faultFile) Chmod(mode os.FileMode) error {
	if err := file.filesystem.step("chmod-file:" + file.path); err != nil {
		return err
	}
	return file.DurableFile.Chmod(mode)
}
func (file *faultFile) Sync() error {
	if err := file.filesystem.step("fsync-file:" + file.path); err != nil {
		return err
	}
	return file.DurableFile.Sync()
}
func (file *faultFile) Close() error {
	err := file.DurableFile.Close()
	return errors.Join(err, file.filesystem.step("close:"+file.path))
}

func tracePosition(trace []string, name string) int {
	for index, value := range trace {
		if value == name {
			return index
		}
	}
	return -1
}

func TestPublicationFollowsCompleteDurableBackup(t *testing.T) {
	root, plan := transactionFixture(t)
	fs := &faultFilesystem{root: root}
	if _, err := (Engine{Filesystem: fs}).Apply(root, plan); err != nil {
		t.Fatal(err)
	}
	backup := tracePosition(fs.trace, "fsync-file:"+preparing+"/journal.json.next")
	privateSync := tracePosition(fs.trace, "sync-dir:"+preparing)
	publication := tracePosition(fs.trace, "rename:"+preparing+"->"+state)
	authored := -1
	for index, name := range fs.trace {
		if strings.HasPrefix(name, "rename:"+state+"/") && !strings.Contains(name, "journal.json") {
			authored = index
			break
		}
	}
	if backup < 0 || privateSync <= backup || publication <= privateSync || authored <= publication {
		t.Fatalf("durability order: %#v", fs.trace)
	}
	t.Logf("durability trace:\n%s", strings.Join(fs.trace, "\n"))
}

func TestFilesystemFailuresKeepEnoughEvidenceToResumeOrArchive(t *testing.T) {
	for _, failure := range []string{
		"write:" + preparing + "/journal.json.next",
		"chmod-file:" + preparing + "/journal.json.next",
		"fsync-file:" + preparing + "/journal.json.next",
		"close:" + preparing + "/journal.json.next",
		"rename:" + preparing + "/journal.json.next->" + preparing + "/journal.json",
		"sync-dir:" + preparing,
		"rename:" + preparing + "->" + state,
		"chmod:" + state + "/directory",
		"write:" + state + "/payload",
		"fsync-file:" + state + "/payload",
		"rename:" + state + "/payload->a.md",
		"remove:old/note",
	} {
		t.Run(failure, func(t *testing.T) {
			root, plan := transactionFixture(t)
			fs := &faultFilesystem{root: root, fail: failure, partialWrite: true}
			if _, err := (Engine{Filesystem: fs}).Apply(root, plan); !errors.Is(err, errComponentFault) {
				t.Fatalf("failure not reached: %v", err)
			}
			t.Logf("injected %s:\n%s", failure, strings.Join(fs.trace, "\n"))
			archived, err := (Engine{}).RecoverUnpublished(root, false)
			if err != nil {
				t.Fatal(err)
			}
			if archived {
				assertSnapshot(t, root, plan.Before)
				return
			}
			if _, err := (Engine{}).Recover(root, false); err != nil {
				t.Fatal(err)
			}
			assertSnapshot(t, root, plan.After)
		})
	}
}

func TestUnpublishedRecoverySynchronizesValidatedBackupBeforePublishing(t *testing.T) {
	root, plan := transactionFixture(t)
	fs := &faultFilesystem{root: root, fail: "fsync-file:" + preparing + "/journal.json.next"}
	if _, err := (Engine{Filesystem: fs}).Apply(root, plan); !errors.Is(err, errComponentFault) {
		t.Fatal(err)
	}
	fs.trace = nil
	if archived, err := (Engine{Filesystem: fs}).RecoverUnpublished(root, false); err != nil || archived {
		t.Fatalf("valid backup not recovered: %t (%v)", archived, err)
	}
	backup := tracePosition(fs.trace, "fsync-file:"+preparing+"/journal.json.next")
	publication := tracePosition(fs.trace, "rename:"+preparing+"->"+state)
	if backup < 0 || publication <= backup {
		t.Fatalf("recovery published an unsynced backup: %#v", fs.trace)
	}
}

func interruptedTransaction(t *testing.T) (string, Plan) {
	t.Helper()
	root, plan := transactionFixture(t)
	engine := Engine{Checkpoint: func(name string) error {
		if name == "apply:2" {
			return errComponentFault
		}
		return nil
	}}
	if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
		t.Fatalf("interruption not reached: %v", err)
	}
	return root, plan
}

func TestRecoveryRetriesEveryFilesystemFailureInBothDirections(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		root, _ := interruptedTransaction(t)
		baseline := &faultFilesystem{root: root}
		if _, err := (Engine{Filesystem: baseline}).Recover(root, rollback); err != nil {
			t.Fatal(err)
		}
		for index, operation := range baseline.trace {
			destination := ""
			if strings.HasPrefix(operation, "rename:"+state+"/payload->") {
				destination = strings.TrimPrefix(operation, "rename:"+state+"/payload->")
			} else if strings.HasPrefix(operation, "remove:") && !strings.HasPrefix(operation, "remove:"+namespace+"/") {
				destination = strings.TrimPrefix(operation, "remove:")
			}
			if destination != "" {
				expected := "sync-dir:" + filepath.ToSlash(filepath.Dir(destination))
				if index+1 >= len(baseline.trace) || baseline.trace[index+1] != expected {
					t.Fatalf("authored change %s lacks parent durability before continuing: %v", operation, baseline.trace)
				}
			}
		}
		t.Logf("rollback=%t trace=%v", rollback, baseline.trace)
		occurrences := map[string]int{}
		for _, operation := range baseline.trace {
			occurrences[operation]++
			ordinal := occurrences[operation]
			t.Run(fmt.Sprintf("rollback=%t/%s/%d", rollback, operation, ordinal), func(t *testing.T) {
				root, plan := interruptedTransaction(t)
				fs := &faultFilesystem{root: root, fail: operation, ordinal: ordinal, partialWrite: true}
				if _, err := (Engine{Filesystem: fs}).Recover(root, rollback); !errors.Is(err, errComponentFault) {
					t.Fatalf("failure not reached: %v; trace=%v", err, fs.trace)
				}
				t.Logf("rollback=%t failure=%s ordinal=%d trace=%v", rollback, operation, ordinal, fs.trace)
				journal, err := (Engine{}).Recover(root, rollback)
				if err != nil {
					t.Fatalf("retry failed: %v; trace=%v", err, fs.trace)
				}
				want, phase := plan.After, "complete"
				if rollback {
					want, phase = plan.Before, "rolled_back"
				}
				if journal.Phase != phase {
					t.Fatalf("phase=%s, want %s", journal.Phase, phase)
				}
				assertSnapshot(t, root, want)
				repeated := &faultFilesystem{root: root}
				if _, err := (Engine{Filesystem: repeated}).Recover(root, rollback); err != nil {
					t.Fatal(err)
				}
				if len(repeated.trace) != 0 {
					t.Fatalf("terminal retry wrote again: %v", repeated.trace)
				}
				assertSnapshot(t, root, want)
			})
		}
	}
}
