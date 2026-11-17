package store

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestLegacyManifestLoads(t *testing.T) {
	for _, test := range []struct {
		name, body string
		paths      []string
		empty      bool
		kind       string
	}{
		{"test_load_manifest_valid", "schema_version: '1'\nprojects: []\n", nil, false, ""},
		{"test_load_manifest_incompatible_schema", "schema_version: '2'\nprojects: []\n", nil, false, "incompatible_schema"},
		{"test_load_manifest_empty_file_returns_default", "", nil, true, ""},
		{"test_load_manifest_whitespace_only_returns_default", "   \n\t\n", nil, true, ""},
		{"test_load_manifest_parse_error", "invalid: yaml: [[[", nil, false, "parse_error"},
		{"test_load_manifest_string_project_paths", "schema_version: '1'\nprojects:\n  - alpha\n  - beta\n  - gamma\n", []string{"./projects/alpha", "./projects/beta", "./projects/gamma"}, false, ""},
		{"test_load_manifest_mixed_project_entries", "schema_version: '1'\nprojects:\n  - alpha\n  - name: beta\n    path: ./projects/beta\n    created_at: \"2026-01-01T00:00:00Z\"\n", []string{"./projects/alpha", "./projects/beta"}, false, ""},
		{"test_load_manifest_schema_alias", "schema: '1'\nprojects: []\n", nil, false, ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "minds.yaml")
			if err := os.WriteFile(path, []byte(test.body), 0600); err != nil {
				t.Fatal(err)
			}
			got, err := ReadManifest(path)
			if test.kind != "" {
				var failure *output.Error
				if !errors.As(err, &failure) || failure.Kind != test.kind {
					t.Fatalf("manifest = %v, want %s", err, test.kind)
				}
				return
			}
			if err != nil || got.SchemaVersion != "1" {
				t.Fatalf("manifest = %+v, %v", got, err)
			}
			if test.empty && len(got.Projects) != 0 {
				t.Fatalf("default projects = %+v", got.Projects)
			}
			if test.paths != nil {
				paths := []string{}
				for _, entry := range got.Projects {
					paths = append(paths, entry.Path)
				}
				if !reflect.DeepEqual(paths, test.paths) {
					t.Fatalf("project paths = %v, want %v", paths, test.paths)
				}
			}
		})
	}
	t.Run("test_save_and_load_manifest", func(t *testing.T) {
		manifest := model.Manifest{SchemaVersion: "1", ProjectsDir: "projects", Projects: []model.ProjectEntry{{Name: "test", Path: "./test", CreatedAt: "2026-04-27T00:00:00Z"}}}
		path := filepath.Join(t.TempDir(), "minds.yaml")
		if err := WriteManifest(path, manifest); err != nil {
			t.Fatal(err)
		}
		got, err := ReadManifest(path)
		if err != nil || len(got.Projects) != 1 || got.Projects[0].Name != "test" {
			t.Fatalf("round trip = %+v, %v", got, err)
		}
	})
	t.Run("test_create_default", func(t *testing.T) {
		got := model.DefaultManifest()
		if got.SchemaVersion != "1" || len(got.Projects) != 0 {
			t.Fatalf("default = %+v", got)
		}
	})
	t.Run("test_project_relpath_default", func(t *testing.T) {
		for _, test := range []struct{ dir, want string }{{"projects", "./projects/foo"}, {".", "./foo"}, {"", "./foo"}, {"/projects/", "./projects/foo"}} {
			if got := ProjectRelativePath(test.dir, "foo"); got != test.want {
				t.Fatalf("relpath(%s) = %s, want %s", test.dir, got, test.want)
			}
		}
	})
}
