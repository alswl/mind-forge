package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"go.yaml.in/yaml/v3"
)

func TestEffectiveConfigSerialization(t *testing.T) {
	dir := t.TempDir()
	config, err := EffectiveConfig(dir, "")
	if err != nil || config.Layout == nil {
		t.Fatalf("repo-free effective configuration: %#v (%v)", config, err)
	}
	for _, format := range []string{"json", "yaml", "unknown"} {
		text, err := SerializeConfig(config, format)
		if err != nil {
			t.Fatal(err)
		}
		var decoded model.MindConfig
		if err := yaml.Unmarshal([]byte(text), &decoded); err != nil {
			t.Fatal(err)
		}
		before, err := json.Marshal(config)
		if err != nil {
			t.Fatal(err)
		}
		after, err := json.Marshal(decoded)
		if err != nil {
			t.Fatal(err)
		}
		if string(before) != string(after) {
			t.Fatalf("%s serialization changed effective configuration\nbefore: %s\nafter: %s", format, before, after)
		}
	}
	entries, err := os.ReadDir(dir)
	if err != nil || len(entries) != 0 {
		t.Fatalf("effective config created state: %v (%v)", entries, err)
	}
}

func TestConfigSchemaPreservesPublishedConstraints(t *testing.T) {
	var schema map[string]any
	if err := json.Unmarshal([]byte(ConfigSchema("json")), &schema); err != nil {
		t.Fatal(err)
	}
	if schema["$schema"] != "http://json-schema.org/draft-07/schema#" || schema["title"] != "MindConfig" {
		t.Fatalf("schema identity changed: %#v", schema)
	}
	properties := schema["properties"].(map[string]any)
	version := properties["schema_version"].(map[string]any)
	if version["type"] != "string" || version["default"] != "1" {
		t.Fatalf("schema version constraint changed: %#v", version)
	}
	if properties["articles"] != true {
		t.Fatal("legacy article configuration must permit arbitrary YAML values")
	}
	for _, name := range []string{"layout", "plugins", "project"} {
		property := properties[name].(map[string]any)
		branches, ok := property["anyOf"].([]any)
		if !ok || len(branches) != 2 || branches[1].(map[string]any)["type"] != "null" {
			t.Errorf("%s must accept an absent/null optional section: %#v", name, property)
		}
	}
	definitions := schema["definitions"].(map[string]any)
	layout := definitions["ContentLayout"].(map[string]any)["enum"].([]any)
	if len(layout) != 2 || layout[0] != "category" || layout[1] != "bundle" {
		t.Errorf("layout choices changed: %#v", layout)
	}
	var yamlSchema map[string]any
	if err := yaml.Unmarshal([]byte(ConfigSchema("yaml")), &yamlSchema); err != nil {
		t.Fatal(err)
	}
	if yamlSchema["title"] != schema["title"] || len(yamlSchema["definitions"].(map[string]any)) != len(definitions) {
		t.Fatal("YAML schema lost definitions")
	}
	if ConfigSchema("unknown") != ConfigSchema("json") {
		t.Fatal("non-YAML schema format must retain JSON fallback")
	}
}

func TestProjectConfigDiscoveryAndNormalization(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "projects", "example")
	child := filepath.Join(project, "nested")
	if err := os.MkdirAll(child, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(project, "mind.yaml")
	raw := "name: example\ndescription: retained\ncreated: '2026-01-01'\npublish:\n  targets:\n  - name: local\n    type: local\n    path: exported\npaths: {docs: chapters}\n"
	if err := os.WriteFile(path, []byte(raw), 0o644); err != nil {
		t.Fatal(err)
	}
	if config, err := LoadProjectConfig(child, ""); err != nil || config != nil {
		t.Fatalf("repo-free lookup recursed: %#v (%v)", config, err)
	}
	config, err := LoadProjectConfig(child, root)
	if err != nil {
		t.Fatal(err)
	}
	if config.Project.Name != "example" || *config.Project.CreatedAt != "2026-01-01" {
		t.Fatalf("project metadata not normalized: %#v", config.Project)
	}
	if (*config.Publish.Targets)[0].Config.(map[string]any)["path"] != "exported" {
		t.Fatal("legacy publish configuration was not normalized")
	}
	layout, warning := ResolveEffectiveLayout(config)
	if layout.Articles != "chapters" || warning != "" {
		t.Fatalf("compatibility paths ignored: %#v %s", layout, warning)
	}
	if actual, _ := os.ReadFile(path); string(actual) != raw {
		t.Fatal("config lookup rewrote source")
	}
	if err := os.WriteFile(path, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	config, err = LoadProjectConfig(child, root)
	if err != nil || config.Project != nil || config.Layout == nil {
		t.Fatalf("empty config must retain explicit defaults: %#v (%v)", config, err)
	}
}

func TestConfigMergePreservesOptionalFallbackAndReplacesCollections(t *testing.T) {
	base := model.DefaultMindConfig()
	name, target := "base", "local"
	base.Name, base.Publish.DefaultTarget = &name, &target
	base.Build.MergeOrder = []string{"base"}
	overlay := model.MindConfig{SchemaVersion: "1", Build: model.DefaultBuildConfig()}
	emptyTargets := []model.PublishTarget{}
	overlay.Publish.Targets = &emptyTargets
	merged := MergeConfig(base, overlay)
	if merged.Name != &name || merged.Publish.DefaultTarget != &target || merged.Publish.Targets == nil || len(*merged.Publish.Targets) != 0 {
		t.Fatalf("optional merge changed: %#v", merged)
	}
	if merged.Layout != nil || len(merged.Build.MergeOrder) != 0 {
		t.Fatal("nonoptional overlay values must replace defaults")
	}
}

func TestLayoutValidationAndConflictOrdering(t *testing.T) {
	root := t.TempDir()
	config := model.DefaultMindConfig()
	config.Paths.Docs = "old"
	config.Build.OutputDir = "old-output"
	layout, warning := ResolveEffectiveLayout(&config)
	if warning != `layout.articles="docs" overrides paths.docs="old"; layout.build_output="outputs" overrides build.output_dir="old-output"` {
		t.Fatalf("conflict warning changed: %s", warning)
	}
	for _, value := range []string{" ", "/absolute", "../escape", "a/../", "sources/./", "existing"} {
		t.Run(value, func(t *testing.T) {
			if err := os.WriteFile(filepath.Join(root, "existing"), nil, 0o644); err != nil {
				t.Fatal(err)
			}
			candidate := layout
			candidate.Articles = value
			if err := ValidateLayout(root, candidate); err == nil {
				t.Fatalf("unsafe layout accepted: %q", value)
			}
		})
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "linked")); err != nil {
		t.Fatal(err)
	}
	layout.Articles = "linked"
	if err := ValidateLayout(root, layout); err == nil || !strings.Contains(err.Error(), "escapes") {
		t.Fatalf("symlink escape accepted: %v", err)
	}
	layout.Articles = "chapter/../chapters"
	if err := ValidateLayout(root, layout); err != nil {
		t.Fatalf("safe normalization rejected: %v", err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "linked-output")); err != nil {
		t.Fatal(err)
	}
	layout.BuildOutput = "linked-output/new"
	if err := ValidateLayout(root, layout); err == nil || !strings.Contains(err.Error(), "escapes") {
		t.Fatalf("symlink escape with missing suffix accepted: %v", err)
	}
}

func TestProjectConfigRejectsInvalidDocuments(t *testing.T) {
	for _, input := range []string{"null", "# comment only", "[one]", "build: [", "schema: '2'", "templates: {Bad: {pattern: x, mode: unknown}}"} {
		t.Run(input, func(t *testing.T) {
			root := t.TempDir()
			if err := os.WriteFile(filepath.Join(root, "mind.yaml"), []byte(input), 0o644); err != nil {
				t.Fatal(err)
			}
			if _, err := LoadProjectConfig(root, root); err == nil {
				t.Fatal("invalid project config accepted")
			}
		})
	}
}
