# Lifecycle persistence and recovery

The lifecycle planner builds a repository inventory and a proposed write set.
The inventory watches repository files, new references, and Additional resources.
It excludes root `.git` and `.mind-forge` runtime state.

Apply revalidates the inventory under an exclusive advisory lock on the repository
root directory inode. CLI write command families share this lock. Dry-run does not
acquire it. External editors do not cooperate with this lock.

Backups and desired contents are fsynced before authored changes. Files and new
directories are staged with their final permissions before being renamed into
place. Preparation occurs in a private `.mind-forge/lifecycle-preparing` directory.
The complete backup is fsynced before that directory becomes the active journal.
No authored changes occur before journal publication.

Recovery accepts only expected before/after states. It rejects conflicting later
edits. Completed recovery retries do not rewrite files. Reversing a completed
transaction requires a new plan. This is a cooperative local recovery protocol,
not multi-file ACID or protection against external editing.

Lifecycle is restricted to supported local filesystems and YAML authority.
It rejects symlinks, hard links, cross-device moves, unsafe permissions, unknown
ownership, and existing or configured Lance authority. It does not verify
references outside the repository.

Additional rename/remove reuse the same planner, reference handling, journal, and
recovery engine. The complete repository safety inventory can reject an operation
because of an unrelated unsafe entry.

See [manage content](../guide/manage-content.md) for the CLI sequence, preparation
recovery states, filesystem list, and legacy incomplete-journal recovery option.
