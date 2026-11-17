package lifecycle

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const (
	namespace   = ".mind-forge"
	preparing   = ".mind-forge/lifecycle-preparing"
	aborted     = ".mind-forge/lifecycle-preparation-aborted"
	state       = ".mind-forge/lifecycle"
	journalPath = ".mind-forge/lifecycle/journal.json"
)

func (engine Engine) exists(path string) bool {
	_, err := engine.Filesystem.Lstat(path)
	return err == nil
}

func (engine Engine) statePath(root string) error {
	rootInfo, err := engine.Filesystem.Lstat(root)
	if err != nil {
		return err
	}
	rootProperties, err := fileProperties(rootInfo)
	if err != nil {
		return err
	}
	for _, relative := range []string{namespace, state, preparing, preparing + "/journal.json", preparing + "/journal.json.next", aborted, journalPath, journalPath + ".next", namespace + "/write.lock", state + "/payload", state + "/directory"} {
		path := filepath.Join(root, relative)
		info, err := engine.Filesystem.Lstat(path)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return rejected("unsafe_path", path)
		}
		properties, err := fileProperties(info)
		if err != nil {
			return err
		}
		if properties.device != rootProperties.device {
			return rejected("cross_device", path)
		}
		if !info.IsDir() && !info.Mode().IsRegular() {
			return rejected("unsafe_path", "non-regular transaction metadata: "+path)
		}
		if info.Mode().IsRegular() && properties.links != 1 {
			return rejected("unsafe_path", "hardlinked transaction metadata: "+path)
		}
	}
	return nil
}

func (engine Engine) lock(root string) (io.Closer, error) {
	if err := engine.statePath(root); err != nil {
		return nil, err
	}
	return engine.AcquireLock(root)
}

func (engine Engine) writeFile(path string, raw []byte, mode uint32, exclusive bool) (result error) {
	if info, err := engine.Filesystem.Lstat(path); err == nil {
		if !info.Mode().IsRegular() {
			return rejected("unsafe_path", path)
		}
		properties, err := fileProperties(info)
		if err != nil {
			return err
		}
		if properties.links != 1 {
			return rejected("unsafe_path", "hardlinked transaction metadata: "+path)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	flags := os.O_WRONLY | os.O_CREATE | os.O_TRUNC
	if exclusive {
		flags = os.O_WRONLY | os.O_CREATE | os.O_EXCL
	}
	file, err := engine.Filesystem.OpenFile(path, flags, 0o600)
	if err != nil {
		return err
	}
	closed := false
	defer func() {
		if !closed {
			result = errors.Join(result, file.Close())
		}
	}()
	count, err := file.Write(raw)
	if err != nil {
		return err
	}
	if count != len(raw) {
		return io.ErrShortWrite
	}
	if err := file.Chmod(unixMode(mode)); err != nil {
		return err
	}
	if err := file.Sync(); err != nil {
		return err
	}
	closed = true
	return file.Close()
}

func (engine Engine) saveJournal(directory string, journal Journal) error {
	raw, err := CanonicalJSON(journal)
	if err != nil {
		return err
	}
	next := filepath.Join(directory, "journal.json.next")
	if err := engine.writeFile(next, raw, 0o600, false); err != nil {
		return err
	}
	if err := engine.Filesystem.Rename(next, filepath.Join(directory, "journal.json")); err != nil {
		return err
	}
	return engine.Filesystem.SyncDir(directory)
}

func (engine Engine) journalFile(directory string) string {
	path := filepath.Join(directory, "journal.json")
	if info, err := engine.Filesystem.Lstat(path); err == nil && info.Mode().IsRegular() {
		return path
	}
	return path + ".next"
}

func (engine Engine) durableBackup(directory, path string) (result error) {
	// A readable complete image can remain after a failed fsync. Re-establish
	// file and directory durability before publishing it or touching authors.
	file, err := engine.Filesystem.OpenFile(path, os.O_RDONLY, 0)
	if err != nil {
		return err
	}
	if err := file.Sync(); err != nil {
		return errors.Join(err, file.Close())
	}
	if err := file.Close(); err != nil {
		return err
	}
	if filepath.Base(path) == "journal.json.next" {
		if err := engine.Filesystem.Rename(path, filepath.Join(directory, "journal.json")); err != nil {
			return err
		}
	}
	return engine.Filesystem.SyncDir(directory)
}

func (engine Engine) ReadJournal(root string) (*Journal, error) {
	engine = engine.defaults()
	if err := engine.statePath(root); err != nil {
		return nil, err
	}
	if !engine.exists(filepath.Join(root, state)) && engine.exists(filepath.Join(root, preparing)) {
		return nil, rejected("recovery_required", "unpublished preparation; use lifecycle resume or rollback (no external plan required)")
	}
	path := engine.journalFile(filepath.Join(root, state))
	if !engine.exists(path) {
		if engine.exists(filepath.Join(root, state)) {
			return nil, rejected("recovery_required", "journal preparation interrupted before a durable journal; authored writes have not begun")
		}
		return nil, nil
	}
	raw, err := engine.Filesystem.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var journal Journal
	if err := json.Unmarshal(raw, &journal); err != nil {
		return nil, rejected("invalid_journal", "incomplete or invalid journal; retain evidence and recover with original plan: "+err.Error())
	}
	if err := journal.Plan.ValidateShape(); err != nil {
		return nil, err
	}
	digest, err := journal.Plan.ComputeDigest()
	if err != nil {
		return nil, err
	}
	if digest != journal.Plan.Digest {
		return nil, rejected("invalid_journal", "digest mismatch")
	}
	return &journal, nil
}

func (engine Engine) CheckRecovery(root string) error {
	journal, err := engine.ReadJournal(root)
	if err != nil {
		return err
	}
	if journal != nil && journal.Phase == "prepared" {
		return rejected("recovery_required", journal.Plan.Digest)
	}
	return nil
}

func (engine Engine) ValidatePlan(root string, plan Plan) error {
	engine = engine.defaults()
	if err := plan.VerifyRoot(root); err != nil {
		return err
	}
	if err := engine.CheckRecovery(root); err != nil {
		return err
	}
	current, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return err
	}
	return plan.VerifyBefore(current)
}

func (engine Engine) Apply(root string, plan Plan) (Journal, error) {
	engine = engine.defaults()
	if err := plan.VerifyRoot(root); err != nil {
		return Journal{}, err
	}
	lock, err := engine.lock(root)
	if err != nil {
		return Journal{}, err
	}
	defer lock.Close()
	previous, err := engine.ReadJournal(root)
	if err != nil {
		return Journal{}, err
	}
	if previous != nil {
		if previous.Plan.Digest == plan.Digest && previous.Phase == "complete" {
			return *previous, nil
		}
		if previous.Phase == "prepared" {
			return Journal{}, rejected("recovery_required", previous.Plan.Digest)
		}
	}
	current, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return Journal{}, err
	}
	if err := plan.VerifyBefore(current); err != nil {
		return Journal{}, err
	}
	if err := engine.Filesystem.MkdirAll(filepath.Join(root, namespace), 0o755); err != nil {
		return Journal{}, err
	}
	if err := engine.Filesystem.SyncDir(root); err != nil {
		return Journal{}, err
	}
	journal := Journal{Plan: plan, Phase: "prepared"}
	active := filepath.Join(root, state)
	if !engine.exists(active) {
		if err := engine.checkpoint("prepare:namespace"); err != nil {
			return Journal{}, err
		}
		private := filepath.Join(root, preparing)
		if err := engine.Filesystem.Mkdir(private, 0o700); err != nil {
			return Journal{}, err
		}
		if err := engine.Filesystem.SyncDir(filepath.Dir(private)); err != nil {
			return Journal{}, err
		}
		if err := engine.checkpoint("prepare:directory"); err != nil {
			return Journal{}, err
		}
		if err := engine.saveJournal(private, journal); err != nil {
			return Journal{}, err
		}
		if err := engine.checkpoint("prepare:backup"); err != nil {
			return Journal{}, err
		}
		if err := engine.Filesystem.Rename(private, active); err != nil {
			return Journal{}, err
		}
		if err := engine.Filesystem.SyncDir(filepath.Dir(active)); err != nil {
			return Journal{}, err
		}
		if err := engine.checkpoint("prepare:publish"); err != nil {
			return Journal{}, err
		}
	} else {
		if err := engine.Filesystem.Chmod(active, 0o700); err != nil {
			return Journal{}, err
		}
		if err := engine.saveJournal(active, journal); err != nil {
			return Journal{}, err
		}
	}
	if err := engine.checkpoint("apply:0"); err != nil {
		return Journal{}, err
	}
	return engine.finish(root, journal, false)
}

func (engine Engine) partial(root string, plan Plan) error {
	current, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return err
	}
	return plan.VerifyPartial(current)
}

func (engine Engine) finish(root string, journal Journal, rollback bool) (Journal, error) {
	plan := journal.Plan
	target := plan.After
	if rollback {
		target = plan.Before
	}
	if err := engine.partial(root, plan); err != nil {
		return Journal{}, err
	}
	current, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return Journal{}, err
	}
	set := map[string]bool{}
	for path := range current {
		set[path] = true
	}
	for path := range target {
		set[path] = true
	}
	paths := make([]string, 0, len(set))
	for path := range set {
		paths = append(paths, path)
	}
	priority := func(path string) (int, int) {
		depth := strings.Count(path, "/")
		if entry, exists := target[path]; exists {
			if entry.Kind == "directory" {
				return 0, depth
			}
			return 1, depth
		}
		return 2, -depth
	}
	sort.Slice(paths, func(a, b int) bool {
		ka, da := priority(paths[a])
		kb, db := priority(paths[b])
		if ka != kb {
			return ka < kb
		}
		if da != db {
			return da < db
		}
		return paths[a] < paths[b]
	})
	step := 0
	for _, path := range paths {
		old, oldExists := current[path]
		desired, exists := target[path]
		if sameEntry(old, oldExists, desired, exists) {
			continue
		}
		if err := engine.partial(root, plan); err != nil {
			return Journal{}, err
		}
		destination, err := safePath(root, path)
		if err != nil {
			return Journal{}, err
		}
		switch {
		case !exists:
			if err := engine.Filesystem.Remove(destination); err != nil {
				return Journal{}, err
			}
		case desired.Kind == "directory":
			if !engine.exists(destination) {
				staging := filepath.Join(root, state, "directory")
				if engine.exists(staging) {
					info, err := engine.Filesystem.Lstat(staging)
					if err != nil {
						return Journal{}, err
					}
					entries, err := engine.Filesystem.ReadDir(staging)
					if err != nil {
						return Journal{}, err
					}
					if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 || len(entries) != 0 {
						return Journal{}, rejected("recovery_conflict", "unknown directory staging content")
					}
					if err := engine.Filesystem.Remove(staging); err != nil {
						return Journal{}, err
					}
				}
				if err := engine.Filesystem.Mkdir(staging, 0o700); err != nil {
					return Journal{}, err
				}
				if err := engine.Filesystem.Chmod(staging, unixMode(desired.Mode)); err != nil {
					return Journal{}, err
				}
				if err := engine.Filesystem.SyncDir(staging); err != nil {
					return Journal{}, err
				}
				if err := engine.Filesystem.Rename(staging, destination); err != nil {
					return Journal{}, err
				}
			} else if err := engine.Filesystem.Chmod(destination, unixMode(desired.Mode)); err != nil {
				return Journal{}, err
			}
			if err := engine.Filesystem.SyncDir(destination); err != nil {
				return Journal{}, err
			}
		default:
			staging := filepath.Join(root, state, "payload")
			if err := engine.writeFile(staging, desired.Bytes, desired.Mode, false); err != nil {
				return Journal{}, err
			}
			if err := engine.Filesystem.Rename(staging, destination); err != nil {
				return Journal{}, err
			}
		}
		if err := engine.Filesystem.SyncDir(filepath.Dir(destination)); err != nil {
			return Journal{}, err
		}
		step++
		if err := engine.checkpoint(fmt.Sprintf("apply:%d", step)); err != nil {
			return Journal{}, err
		}
	}
	final, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return Journal{}, err
	}
	if err := (Plan{Before: target}).VerifyBefore(final); err != nil {
		return Journal{}, rejected("recovery_conflict", "final inventory differs")
	}
	journal.Phase = "complete"
	if rollback {
		journal.Phase = "rolled_back"
	}
	if err := engine.saveJournal(filepath.Join(root, state), journal); err != nil {
		return Journal{}, err
	}
	if err := engine.checkpoint(fmt.Sprintf("apply:%d", step+1)); err != nil {
		return Journal{}, err
	}
	return journal, nil
}
