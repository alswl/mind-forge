package lifecycle

import (
	"encoding/json"
	"path/filepath"
	"strconv"
)

func (engine Engine) Recover(root string, rollback bool) (Journal, error) {
	engine = engine.defaults()
	lock, err := engine.lock(root)
	if err != nil {
		return Journal{}, err
	}
	defer lock.Close()
	journal, err := engine.ReadJournal(root)
	if err != nil {
		return Journal{}, err
	}
	if journal == nil {
		return Journal{}, rejected("journal_missing", "no transaction to recover")
	}
	if err := journal.Plan.VerifyRoot(root); err != nil {
		return Journal{}, err
	}
	desired := "complete"
	if rollback {
		desired = "rolled_back"
	}
	if journal.Phase == desired {
		return *journal, nil
	}
	if journal.Phase != "prepared" {
		return Journal{}, rejected("transaction_finished", "completed transactions cannot change direction")
	}
	active := filepath.Join(root, state)
	if err := engine.durableBackup(active, engine.journalFile(active)); err != nil {
		return Journal{}, err
	}
	if err := engine.partial(root, journal.Plan); err != nil {
		return Journal{}, err
	}
	return engine.finish(root, *journal, rollback)
}

// RecoverUnpublished returns true when preparation was archived without any
// authored changes. The caller can then retry the original operation. False
// means normal journal recovery should continue.
func (engine Engine) RecoverUnpublished(root string, rollback bool) (bool, error) {
	engine = engine.defaults()
	lock, err := engine.lock(root)
	if err != nil {
		return false, err
	}
	defer lock.Close()
	if engine.exists(filepath.Join(root, state)) {
		return false, nil
	}
	private := filepath.Join(root, preparing)
	if !engine.exists(private) {
		return engine.exists(filepath.Join(root, aborted)), nil
	}
	entries, err := engine.Filesystem.ReadDir(private)
	if err != nil {
		return false, err
	}
	for _, entry := range entries {
		if entry.Name() != "journal.json" && entry.Name() != "journal.json.next" {
			return false, rejected("invalid_journal", "unknown preparation entry; evidence preserved")
		}
	}
	path := filepath.Join(private, "journal.json")
	if !engine.exists(path) {
		path += ".next"
	}
	var journal *Journal
	if engine.exists(path) {
		raw, err := engine.Filesystem.ReadFile(path)
		if err != nil {
			return false, err
		}
		var decoded Journal
		if err := json.Unmarshal(raw, &decoded); err == nil {
			journal = &decoded
		}
	}
	if journal != nil {
		if err := journal.Plan.VerifyRoot(root); err != nil {
			return false, err
		}
		if journal.Phase != "prepared" {
			return false, rejected("invalid_journal", "unpublished journal must be prepared")
		}
		if !rollback {
			if err := engine.durableBackup(private, path); err != nil {
				return false, err
			}
			if err := engine.Filesystem.Rename(private, filepath.Join(root, state)); err != nil {
				return false, err
			}
			if err := engine.Filesystem.SyncDir(filepath.Dir(private)); err != nil {
				return false, err
			}
			return false, nil
		}
	}
	// The receipt precedes evidence archival so interruption between those two
	// durable operations remains distinguishable from a transaction with no intent.
	receipt := filepath.Join(root, aborted)
	if !engine.exists(receipt) {
		if err := engine.writeFile(receipt, nil, 0o600, true); err != nil {
			return false, err
		}
		if err := engine.Filesystem.SyncDir(filepath.Dir(receipt)); err != nil {
			return false, err
		}
	}
	var archive string
	for serial := 0; ; serial++ {
		archive = filepath.Join(root, namespace, "lifecycle-preparation-evidence-"+strconv.Itoa(serial))
		if !engine.exists(archive) {
			break
		}
	}
	if err := engine.Filesystem.Rename(private, archive); err != nil {
		return false, err
	}
	if err := engine.Filesystem.SyncDir(filepath.Dir(private)); err != nil {
		return false, err
	}
	return true, nil
}

func (engine Engine) RecoverPreparation(root string, plan Plan, rollback bool) (Journal, error) {
	engine = engine.defaults()
	if err := plan.VerifyRoot(root); err != nil {
		return Journal{}, err
	}
	lock, err := engine.lock(root)
	if err != nil {
		return Journal{}, err
	}
	defer lock.Close()
	active := filepath.Join(root, state)
	info, statError := engine.Filesystem.Lstat(active)
	if engine.exists(filepath.Join(root, journalPath)) || statError != nil || !info.IsDir() {
		return Journal{}, rejected("invalid_journal", "initial preparation recovery requires an interrupted initial journal")
	}
	if journal, err := engine.ReadJournal(root); err == nil && journal != nil {
		return Journal{}, rejected("recovery_required", "complete backup exists; use resume or rollback")
	}
	entries, err := engine.Filesystem.ReadDir(active)
	if err != nil {
		return Journal{}, err
	}
	for _, entry := range entries {
		if entry.Name() != "journal.json.next" {
			return Journal{}, rejected("invalid_journal", "unknown preparation evidence; refusing to replace it")
		}
	}
	current, err := engine.Inventory(root, plan.Watches)
	if err != nil {
		return Journal{}, err
	}
	if err := plan.VerifyBefore(current); err != nil {
		return Journal{}, rejected("recovery_conflict", "initial recovery requires unchanged complete before inventory")
	}
	next := filepath.Join(root, journalPath+".next")
	if engine.exists(next) {
		raw, err := engine.Filesystem.ReadFile(next)
		if err != nil {
			return Journal{}, err
		}
		evidence := filepath.Join(root, namespace, "lifecycle-preparation-"+Digest(raw)+".json")
		if engine.exists(evidence) {
			return Journal{}, rejected("path_conflict", evidence)
		}
		if err := engine.Filesystem.Rename(next, evidence); err != nil {
			return Journal{}, err
		}
		if err := engine.Filesystem.SyncDir(filepath.Join(root, namespace)); err != nil {
			return Journal{}, err
		}
	}
	journal := Journal{Plan: plan, Phase: "prepared"}
	if err := engine.Filesystem.Chmod(active, 0o700); err != nil {
		return Journal{}, err
	}
	if err := engine.saveJournal(active, journal); err != nil {
		return Journal{}, err
	}
	return engine.finish(root, journal, rollback)
}
