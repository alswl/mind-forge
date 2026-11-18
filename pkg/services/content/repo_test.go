package content

import (
	"os"
	"path/filepath"
	"testing"
)

func TestInitializeRepositoryIsIdempotent(t *testing.T) {
	parent := t.TempDir()
	target := filepath.Join(parent, "repo")
	report, err := InitializeRepository(target)
	if err != nil || !report.Created || report.AlreadyExisted || len(report.CreatedFiles) != 1 || len(report.CreatedDirectories) != 1 {
		t.Fatalf("init: %#v (%v)", report, err)
	}
	path := filepath.Join(target, "minds.yaml")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	before, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	report, err = InitializeRepository(target)
	after, statError := os.Stat(path)
	if statError != nil {
		t.Fatal(statError)
	}
	if err != nil || report.Created || !report.AlreadyExisted || len(report.Skipped) != 1 {
		t.Fatalf("idempotent init: %#v (%v)", report, err)
	}
	if current, err := os.ReadFile(path); err != nil || string(current) != string(raw) || !before.ModTime().Equal(after.ModTime()) {
		t.Fatal("idempotent init rewrote the manifest")
	}
}

func TestInitializeRepositoryRefusesUnsafeTargets(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "nonempty/keep.md", "# Keep\n")
	putContent(t, root, "file", "keep")
	putContent(t, root, "malformed/minds.yaml", "[broken")
	for _, path := range []string{filepath.Join(root, "nonempty"), filepath.Join(root, "file"), filepath.Join(root, "missing/child"), filepath.Join(root, "malformed")} {
		if _, err := InitializeRepository(path); err == nil {
			t.Errorf("unsafe init %q accepted", path)
		}
	}
	if got, err := os.ReadFile(filepath.Join(root, "nonempty/keep.md")); err != nil || string(got) != "# Keep\n" {
		t.Fatal("nonempty content changed")
	}
	repo := filepath.Join(root, "valid")
	if _, err := InitializeRepository(repo); err != nil {
		t.Fatal(err)
	}
	if _, err := InitializeRepository(filepath.Join(repo, "nested")); err == nil {
		t.Fatal("nested repository accepted")
	}
	if _, err := os.Stat(filepath.Join(repo, "nested")); !os.IsNotExist(err) {
		t.Fatal("nested refusal wrote a target directory")
	}
}
