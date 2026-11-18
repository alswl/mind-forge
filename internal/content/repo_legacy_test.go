package content

import (
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

func TestLegacyRepositoryClassification(t *testing.T) {
	for _, name := range []string{"test_classify_new_directory_valid_parent", "test_classify_new_directory_parent_missing", "test_classify_new_directory_parent_is_file", "test_classify_existing_repo", "test_classify_existing_empty_directory", "test_classify_unsafe_non_empty_directory", "test_classify_invalid_file_target", "test_classify_malformed_manifest", "test_classify_incompatible_schema_manifest", "test_classify_invalid_path_dotdot"} {
		t.Run(name, func(t *testing.T) {
			parent := t.TempDir()
			target := parent
			want := ExistingEmptyDirectory
			fragment := ""
			switch name {
			case "test_classify_new_directory_valid_parent":
				target, want = filepath.Join(parent, "new-dir"), NewDirectory
			case "test_classify_new_directory_parent_missing":
				target, fragment = filepath.Join(parent, "missing-parent", "child"), "parent directory"
			case "test_classify_new_directory_parent_is_file":
				putContent(t, parent, "not-a-dir", "content")
				target, fragment = filepath.Join(parent, "not-a-dir", "child"), "not a directory"
			case "test_classify_existing_repo":
				putContent(t, parent, "minds.yaml", "schema_version: '1'\nprojects: []\n")
				want = ExistingRepository
			case "test_classify_unsafe_non_empty_directory":
				putContent(t, parent, "readme.txt", "notes")
				want = UnsafeNonEmptyDirectory
			case "test_classify_invalid_file_target":
				putContent(t, parent, "a-file.txt", "content")
				target, want = filepath.Join(parent, "a-file.txt"), InvalidFileTarget
			case "test_classify_malformed_manifest":
				putContent(t, parent, "minds.yaml", "invalid: yaml: [[[")
				want = MalformedManifest
			case "test_classify_incompatible_schema_manifest":
				putContent(t, parent, "minds.yaml", "schema_version: '99'\nprojects: []\n")
				want = MalformedManifest
			case "test_classify_invalid_path_dotdot":
				target, want = "..", InvalidPath
			}
			got, err := ClassifyRepoTarget(target)
			if fragment != "" {
				if err == nil || !strings.Contains(err.Error(), fragment) {
					t.Fatalf("classification = %v, %v, want error %s", got, err, fragment)
				}
				return
			}
			if err != nil || got != want {
				t.Fatalf("classification = %v, %v, want %v", got, err, want)
			}
		})
	}
}

func TestLegacyRepositoryInitialization(t *testing.T) {
	for _, name := range []string{"test_init_repo_existing_repo_is_idempotent", "test_init_repo_in_empty_directory_creates_files", "test_init_repo_unsafe_non_empty_refused", "test_init_repo_invalid_file_target_refused", "test_init_repo_new_directory_with_cleanup", "test_validate_not_nested_detects_parent_repo", "test_validate_not_nested_ok_when_no_parent_repo"} {
		t.Run(name, func(t *testing.T) {
			parent := t.TempDir()
			target := parent
			fragment := ""
			switch name {
			case "test_init_repo_existing_repo_is_idempotent":
				putContent(t, parent, "minds.yaml", "schema: '1'\nprojects: []\n")
				if err := os.Mkdir(filepath.Join(parent, "projects"), 0700); err != nil {
					t.Fatal(err)
				}
			case "test_init_repo_unsafe_non_empty_refused":
				putContent(t, parent, "readme.txt", "notes")
				fragment = "not empty"
			case "test_init_repo_invalid_file_target_refused":
				putContent(t, parent, "a-file.txt", "content")
				target, fragment = filepath.Join(parent, "a-file.txt"), "file"
			case "test_init_repo_new_directory_with_cleanup":
				target = filepath.Join(parent, "new-repo")
			case "test_validate_not_nested_detects_parent_repo":
				putContent(t, parent, "minds.yaml", "schema: '1'\nprojects: []\n")
				target, fragment = filepath.Join(parent, "subdir"), "inside an existing Mind Repo"
				if err := os.Mkdir(target, 0700); err != nil {
					t.Fatal(err)
				}
			case "test_validate_not_nested_ok_when_no_parent_repo":
				target = filepath.Join(parent, "subdir")
				if err := os.Mkdir(target, 0700); err != nil {
					t.Fatal(err)
				}
			}
			report, err := InitializeRepository(target)
			if fragment != "" {
				if err == nil || !strings.Contains(err.Error(), fragment) {
					t.Fatalf("initialize = %+v, %v, want error %s", report, err, fragment)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if name == "test_init_repo_existing_repo_is_idempotent" {
				if report.Created || !report.AlreadyExisted || len(report.CreatedFiles) != 0 || !slices.Contains(report.Skipped, "minds.yaml") {
					t.Fatalf("idempotence = %+v", report)
				}
				return
			}
			if name == "test_validate_not_nested_ok_when_no_parent_repo" {
				return
			}
			if !report.Created || report.AlreadyExisted {
				t.Fatalf("creation = %+v", report)
			}
			if _, err := os.Stat(filepath.Join(target, "minds.yaml")); err != nil {
				t.Fatal(err)
			}
			info, err := os.Stat(filepath.Join(target, "projects"))
			if err != nil || !info.IsDir() {
				t.Fatalf("projects = %v, %v", info, err)
			}
			if name == "test_init_repo_in_empty_directory_creates_files" && (!slices.Contains(report.CreatedFiles, "minds.yaml") || !slices.Contains(report.CreatedDirectories, "projects")) {
				t.Fatalf("created paths = %+v", report)
			}
		})
	}
}

func TestLegacyRepositoryProjectsDirectory(t *testing.T) {
	for _, test := range []struct{ name, body, want string }{
		{"test_projects_dir_for_default_when_missing", "", "projects"},
		{"test_projects_dir_for_reads_explicit_value", "schema_version: '1'\nprojects_dir: work\nprojects: []\n", "work"},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			if test.body != "" {
				putContent(t, root, "minds.yaml", test.body)
			}
			got, err := loadManifest(root)
			if err != nil || got.ProjectsDir != test.want {
				t.Fatalf("projects dir = %q, %v, want %q", got.ProjectsDir, err, test.want)
			}
		})
	}
}
