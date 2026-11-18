package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"go.yaml.in/yaml/v3"
)

func TestLegacyConfigMerge(t *testing.T) {
	for _, name := range []string{"test_merge_base_only", "test_merge_overlay_wins_scalar", "test_merge_array_replacement", "test_merge_project_none", "test_merge_project_overlay_some"} {
		t.Run(name, func(t *testing.T) {
			base, overlay := model.DefaultMindConfig(), model.DefaultMindConfig()
			switch name {
			case "test_merge_overlay_wins_scalar":
				overlay.Build.OutputDir = "_out"
			case "test_merge_array_replacement":
				base.Source.ScanPaths = []string{"a"}
				overlay.Source.ScanPaths = []string{"b"}
			case "test_merge_project_overlay_some":
				created := "2026-01-01T00:00:00Z"
				overlay.Project = &model.ProjectMeta{Name: "my-project", CreatedAt: &created}
			}
			got := MergeConfig(base, overlay)
			switch name {
			case "test_merge_base_only":
				if got.SchemaVersion != "1" {
					t.Fatal(got.SchemaVersion)
				}
			case "test_merge_overlay_wins_scalar":
				if got.Build.OutputDir != "_out" {
					t.Fatal(got.Build.OutputDir)
				}
			case "test_merge_array_replacement":
				if !reflect.DeepEqual(got.Source.ScanPaths, model.JSONArray[string]{"b"}) {
					t.Fatal(got.Source.ScanPaths)
				}
			case "test_merge_project_none":
				if got.Project != nil {
					t.Fatal("absent project populated")
				}
			case "test_merge_project_overlay_some":
				if got.Project == nil || got.Project.Name != "my-project" {
					t.Fatalf("project overlay lost: %+v", got.Project)
				}
			}
		})
	}
}

func TestLegacyConfigSerialization(t *testing.T) {
	for _, test := range []struct{ name, format string }{{"test_to_yaml_roundtrip", "yaml"}, {"test_to_json_roundtrip", "json"}} {
		t.Run(test.name, func(t *testing.T) {
			got, err := SerializeConfig(model.DefaultMindConfig(), test.format)
			if err != nil || !strings.Contains(got, "schema_version") {
				t.Fatalf("serialized config: %q %v", got, err)
			}
		})
	}
	t.Run("test_to_json_schema_valid", func(t *testing.T) {
		raw := ConfigSchema("json")
		var schema map[string]any
		if err := json.Unmarshal([]byte(raw), &schema); err != nil {
			t.Fatal(err)
		}
		if _, ok := schema["properties"]; !ok {
			t.Fatal("schema properties absent")
		}
		if _, ok := schema["definitions"]; !ok {
			t.Fatal("schema definitions absent")
		}
		if !strings.Contains(raw, "github_pages") {
			t.Fatal("publish target enum absent")
		}
	})
}

func TestLegacyConfigLayoutResolution(t *testing.T) {
	for _, name := range []string{"test_resolve_default_layout", "test_effective_layout_fields_order_is_deterministic", "test_paths_compat_maps_to_layout", "test_build_output_dir_maps_to_layout", "test_canonical_layout_wins_over_paths", "test_layout_docs_alias_maps_to_articles", "test_layout_output_dir_alias_maps_to_build_output"} {
		t.Run(name, func(t *testing.T) {
			c := model.DefaultMindConfig()
			switch name {
			case "test_paths_compat_maps_to_layout":
				c.Layout = nil
				c.Paths.Docs, c.Paths.Sources, c.Paths.Assets = "notes", "refs", "media"
			case "test_build_output_dir_maps_to_layout":
				c.Layout = nil
				c.Build.OutputDir = "dist"
			case "test_canonical_layout_wins_over_paths", "test_layout_docs_alias_maps_to_articles", "test_layout_output_dir_alias_maps_to_build_output":
				inputs := map[string]string{
					"test_canonical_layout_wins_over_paths":             "schema_version: '1'\nlayout:\n  articles: canon\npaths:\n  docs: compat\n",
					"test_layout_docs_alias_maps_to_articles":           "schema_version: '1'\nlayout:\n  docs: legacy_name\n",
					"test_layout_output_dir_alias_maps_to_build_output": "schema_version: '1'\nlayout:\n  output_dir: _myoutput\n",
				}
				if err := yaml.Unmarshal([]byte(inputs[name]), &c); err != nil {
					t.Fatal(err)
				}
			}
			got, _ := ResolveEffectiveLayout(&c)
			switch name {
			case "test_resolve_default_layout":
				if got.Articles != "docs" || got.Sources != "sources" || got.Assets != "assets" || got.Templates != "templates" || got.BuildOutput != "outputs" {
					t.Fatalf("default layout changed: %+v", got)
				}
			case "test_effective_layout_fields_order_is_deterministic":
				raw, err := yaml.Marshal(got)
				if err != nil {
					t.Fatal(err)
				}
				last := -1
				for _, key := range []string{"articles:", "sources:", "assets:", "templates:", "build_output:"} {
					pos := strings.Index(string(raw), key)
					if pos <= last {
						t.Fatalf("layout order changed at %s: %s", key, raw)
					}
					last = pos
				}
			case "test_paths_compat_maps_to_layout":
				if got.Articles != "notes" || got.Sources != "refs" || got.Assets != "media" {
					t.Fatalf("compat layout lost: %+v", got)
				}
			case "test_build_output_dir_maps_to_layout":
				if got.BuildOutput != "dist" {
					t.Fatal(got.BuildOutput)
				}
			case "test_canonical_layout_wins_over_paths":
				if got.Articles != "canon" {
					t.Fatal(got.Articles)
				}
			case "test_layout_docs_alias_maps_to_articles":
				if c.Layout == nil || c.Layout.Articles == nil || *c.Layout.Articles != "legacy_name" {
					t.Fatal("docs alias lost")
				}
			case "test_layout_output_dir_alias_maps_to_build_output":
				if c.Layout == nil || c.Layout.BuildOutput == nil || *c.Layout.BuildOutput != "_myoutput" {
					t.Fatal("output_dir alias lost")
				}
			}
		})
	}
}

func TestLegacyConfigLayoutValidation(t *testing.T) {
	for _, test := range []struct {
		name, articles, sources string
		wants                   []string
		file                    bool
	}{
		{"test_validate_empty_layout_value", "docs", "", []string{"layout.sources", "empty"}, false},
		{"test_validate_absolute_path", "/etc/passwd", "sources", []string{"layout.articles", "absolute"}, false},
		{"test_validate_escape_path", "../outside", "sources", []string{"layout.articles", "escapes"}, false},
		{"test_validate_allows_parent_dir_text_in_segment_name", "notes..drafts", "sources", nil, false},
		{"test_validate_duplicate_paths", "content", "content", []string{"layout.articles", "layout.sources", "same path"}, false},
		{"test_validate_duplicate_paths_after_normalization", "docs", "./docs", []string{"layout.articles", "layout.sources", "same path"}, false},
		{"test_validate_file_path", "existing_file", "sources", []string{"layout.articles", "file"}, true},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			if test.file {
				if err := os.WriteFile(filepath.Join(root, "existing_file"), []byte("hello"), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			err := ValidateLayout(root, model.EffectiveLayout{Articles: test.articles, Sources: test.sources, Assets: "assets", Templates: "templates", BuildOutput: "outputs"})
			if len(test.wants) == 0 {
				if err != nil {
					t.Fatal(err)
				}
				return
			}
			if err == nil {
				t.Fatal("invalid layout accepted")
			}
			for _, part := range test.wants {
				if !strings.Contains(err.Error(), part) {
					t.Fatalf("layout diagnostic missing %q: %v", part, err)
				}
			}
		})
	}
	t.Run("test_resolve_and_validate_passes_for_valid_layout", func(t *testing.T) {
		c := model.DefaultMindConfig()
		if _, err := ResolveAndValidateLayout(t.TempDir(), &c); err != nil {
			t.Fatal(err)
		}
		if c.Layout == nil || c.Layout.Articles == nil || *c.Layout.Articles != "docs" {
			t.Fatal("validated layout absent")
		}
	})
}
