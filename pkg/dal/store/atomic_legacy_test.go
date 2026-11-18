package store

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLegacyAtomicPublication(t *testing.T) {
	t.Run("pending_file_rename_restores_replaced_target_on_rollback", func(t *testing.T) {
		root := t.TempDir()
		old, target := filepath.Join(root, "old.md"), filepath.Join(root, "new.md")
		for path, body := range map[string]string{old: "source contents", target: "previous destination"} {
			if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
				t.Fatal(err)
			}
		}
		pending, err := BeginRename(old, target)
		if err != nil {
			t.Fatal(err)
		}
		if err := pending.Rollback(); err != nil {
			t.Fatal(err)
		}
		for path, want := range map[string]string{old: "source contents", target: "previous destination"} {
			got, err := os.ReadFile(path)
			if err != nil || string(got) != want {
				t.Fatalf("rollback %s = %q, %v; want %q", path, got, err, want)
			}
		}
	})
	for _, name := range []string{"atomic_write_directory_happy_path", "atomic_write_directory_rollback_on_mid_write_failure", "atomic_write_directory_target_already_exists"} {
		t.Run(name, func(t *testing.T) {
			root := t.TempDir()
			target := filepath.Join(root, "my-article")
			files := map[string][]byte{"01-opening.md": []byte("# Title\n"), "02-summary.md": []byte("## Summary\nBody\n")}
			if name == "atomic_write_directory_rollback_on_mid_write_failure" {
				delete(files, "02-summary.md")
				files["bad\x00name.md"] = []byte("nope")
			}
			if name == "atomic_write_directory_target_already_exists" {
				if err := os.Mkdir(target, 0o755); err != nil {
					t.Fatal(err)
				}
			}
			err := CreateDirectory(target, files)
			switch name {
			case "atomic_write_directory_happy_path":
				if err != nil {
					t.Fatal(err)
				}
				info, err := os.Stat(target)
				if err != nil || !info.IsDir() {
					t.Fatalf("target directory absent: %v", err)
				}
				for file, want := range files {
					got, err := os.ReadFile(filepath.Join(target, file))
					if err != nil || string(got) != string(want) {
						t.Fatalf("published %s = %q, %v", file, got, err)
					}
				}
			case "atomic_write_directory_rollback_on_mid_write_failure":
				if err == nil {
					t.Fatal("invalid NUL filename did not fail")
				}
				if _, err := os.Stat(target); !os.IsNotExist(err) {
					t.Fatalf("failed write published target: %v", err)
				}
			}
			entries, err := os.ReadDir(root)
			if err != nil {
				t.Fatal(err)
			}
			for _, entry := range entries {
				if strings.HasPrefix(entry.Name(), ".") {
					t.Fatalf("staging directory leaked: %s", entry.Name())
				}
			}
		})
	}
}
