package model

import (
	"encoding/json"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestLegacyManifestModels(t *testing.T) {
	for _, test := range []struct {
		name, input string
		count       int
	}{
		{"test_deserialize_string_project_entry", "schema_version: '1'\nprojects:\n  - alpha\n  - beta\n", 2},
		{"test_deserialize_object_project_entry", "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: '2026-01-01T00:00:00Z'\n", 1},
		{"test_deserialize_mixed_project_entries", "schema_version: '1'\nprojects:\n  - alpha\n  - name: beta\n    path: ./projects/beta\n    created_at: '2026-01-01T00:00:00Z'\n  - gamma\n", 3},
		{"test_deserialize_schema_alias", "schema: '1'\nprojects: []\n", 0},
		{"test_deserialize_missing_schema_version_defaults_to_1", "projects:\n  - alpha\n", 1},
	} {
		t.Run(test.name, func(t *testing.T) {
			var m Manifest
			if err := yaml.Unmarshal([]byte(test.input), &m); err != nil {
				t.Fatal(err)
			}
			legacyConfigEqual(t, len(m.Projects), test.count)
			legacyConfigEqual(t, m.SchemaVersion, "1")
			if test.name == "test_deserialize_string_project_entry" {
				legacyConfigEqual(t, m.Projects[0].Name, "alpha")
				legacyConfigEqual(t, m.Projects[0].Path, "alpha")
				legacyConfigEqual(t, m.Projects[1].Name, "beta")
			}
			if test.name == "test_deserialize_object_project_entry" {
				legacyConfigEqual(t, m.Projects[0].Name, "alpha")
				legacyConfigEqual(t, m.Projects[0].Path, "./projects/alpha")
			}
		})
	}
	t.Run("test_serialize_project_entry_keeps_object_form", func(t *testing.T) {
		raw, err := yaml.Marshal(ProjectEntry{Name: "alpha", Path: "./projects/alpha", CreatedAt: "2026-01-01T00:00:00Z"})
		if err != nil || !strings.Contains(string(raw), "name: alpha") || !strings.Contains(string(raw), "path:") || strings.HasPrefix(string(raw), "alpha\n") {
			t.Fatalf("project entry wire changed: %s %v", raw, err)
		}
	})
}

func TestLegacyArticleModels(t *testing.T) {
	for _, test := range []struct{ name, input, kind string }{
		{"article_without_template_origin_round_trips_byte_identical", "title: Test Article\nproject: my-project\ntype: blog\narticle_path: docs/test.md\nstatus: draft\ncreated_at: '2026-05-15T00:00:00Z'\nupdated_at: '2026-05-15T00:00:00Z'\n", "blog"},
		{"article_type_blank_round_trips", "title: T\ntype: blank\n", "blank"},
		{"article_type_default_is_blank", "title: T\n", "blank"},
		{"article_type_blog_still_deserializes", "title: Old\nproject: p\ntype: blog\narticle_path: docs/old.md\nstatus: draft\ncreated_at: '2026-01-01T00:00:00Z'\nupdated_at: '2026-01-01T00:00:00Z'\n", "blog"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var a Article
			if err := yaml.Unmarshal([]byte(test.input), &a); err != nil {
				t.Fatal(err)
			}
			legacyConfigEqual(t, a.Type, test.kind)
			raw, err := yaml.Marshal(a)
			if err != nil {
				t.Fatal(err)
			}
			if test.name == "article_without_template_origin_round_trips_byte_identical" && (a.TemplateOrigin != nil || strings.Contains(string(raw), "template_origin")) {
				t.Fatalf("absent template_origin populated: %s", raw)
			}
			if test.name == "article_type_blank_round_trips" && !strings.Contains(string(raw), "type: blank") {
				t.Fatalf("blank type spelling changed: %s", raw)
			}
		})
	}
	t.Run("article_with_template_origin_serializes", func(t *testing.T) {
		a := Article{Title: "Daily Report", Project: "my-project", Type: "blog", Path: "outputs/2026-05/2026-05-15.md", Status: "draft", CreatedAt: "2026-05-15T00:00:00Z", UpdatedAt: "2026-05-15T00:00:00Z", TemplateOrigin: &TemplateOrigin{TemplateName: "daily_report", SlotValue: "2026-05-15"}}
		v := legacyDomainJSON(t, a)
		origin, ok := v["template_origin"].(map[string]any)
		if !ok {
			t.Fatalf("template_origin absent: %#v", v)
		}
		legacyConfigEqual(t, origin["template_name"], "daily_report")
		legacyConfigEqual(t, origin["slot_value"], "2026-05-15")
	})
}

func legacyDomainJSON(t *testing.T, value any) map[string]any {
	t.Helper()
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	var v map[string]any
	if err := json.Unmarshal(raw, &v); err != nil {
		t.Fatal(err)
	}
	return v
}

func TestLegacyPublishModels(t *testing.T) {
	t.Run("local_outcome_target_type_is_local", func(t *testing.T) {
		v := legacyDomainJSON(t, PublishRunOutcome{Local: &LocalRunOutcome{TargetName: "tgt", Article: "a", Source: "/a", Destination: "/b", SizeBytes: 12}})
		legacyConfigEqual(t, v["target_type"], "local")
		legacyConfigEqual(t, v["dry_run"], false)
	})
	for _, name := range []string{"yuque_prompt_outcome_target_type_is_kebab_case", "yuque_prompt_outcome_with_destination_serializes_path"} {
		t.Run(name, func(t *testing.T) {
			o := YuquePromptRunOutcome{TargetName: "tgt", Article: "a", ArticlePath: "docs/a.md", BuildArtifactPath: "/p/_build/a.md", Content: "hello", Prompt: "Please publish ...", Envelope: map[string]any{}, SuggestedUpdateCommand: "mf publish update a --target tgt --status published --target-url <URL>", DryRun: true}
			if name == "yuque_prompt_outcome_with_destination_serializes_path" {
				o.Destination = stringPointerForLegacy("outputs/a.yuque.md")
				o.DryRun = false
			}
			v := legacyDomainJSON(t, PublishRunOutcome{YuquePrompt: &o})
			if o.Destination != nil {
				legacyConfigEqual(t, v["destination"], "outputs/a.yuque.md")
				return
			}
			legacyConfigEqual(t, v["target_type"], "yuque-prompt")
			legacyConfigEqual(t, v["dry_run"], true)
			legacyConfigEqual(t, v["envelope"], map[string]any{})
			transforms, ok := v["transforms"].(map[string]any)
			if !ok {
				t.Fatal("transforms absent")
			}
			legacyConfigEqual(t, transforms["svg_png_replaced"], []any{})
			legacyConfigEqual(t, transforms["svg_png_missing"], []any{})
			if v["destination"] != nil {
				t.Fatal("absent destination unexpectedly populated")
			}
		})
	}
	t.Run("update_action_serializes_snake_case", func(t *testing.T) {
		for _, action := range []string{"created", "updated"} {
			legacyConfigEqual(t, legacyDomainJSON(t, PublishUpdateOutcome{Action: action})["action"], action)
		}
	})
	t.Run("update_outcome_includes_record_with_null_published_at", func(t *testing.T) {
		v := legacyDomainJSON(t, PublishUpdateOutcome{Article: "a", TargetName: "tgt", Action: "created", Record: PublishRecord{Path: "docs/a.md", TargetName: "tgt", Status: "draft"}})
		legacyConfigEqual(t, v["action"], "created")
		record := v["record"].(map[string]any)
		legacyConfigEqual(t, record["status"], "draft")
		for _, key := range []string{"published_at", "target_url"} {
			value, ok := record[key]
			if !ok || value != nil {
				t.Fatalf("record[%s] must be explicit null: %#v", key, record)
			}
		}
	})
}

func TestLegacyProjectModels(t *testing.T) {
	for _, test := range []struct {
		name  string
		entry ProjectListEntry
	}{
		{"test_project_list_entry_serialization", ProjectListEntry{Name: "alpha", Path: "./alpha", CreatedAt: "2026-04-30T12:00:00Z", DocumentCount: 5, LastActivityAt: stringPointerForLegacy("2026-04-30T12:15:00Z")}},
		{"test_project_list_entry_null_last_activity", ProjectListEntry{Name: "empty", Path: "./empty", CreatedAt: "2026-04-30T12:00:00Z"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			v := legacyDomainJSON(t, test.entry)
			if test.entry.LastActivityAt == nil {
				value, ok := v["last_activity_at"]
				if !ok || value != nil {
					t.Fatal("last_activity_at must be explicit null")
				}
				return
			}
			legacyConfigEqual(t, v["name"], "alpha")
			legacyConfigEqual(t, v["path"], "./alpha")
			legacyConfigEqual(t, v["document_count"], float64(5))
			value, ok := v["archived_at"]
			if !ok || value != nil {
				t.Fatal("archived_at must be explicit null")
			}
			legacyConfigEqual(t, v["last_activity_at"], "2026-04-30T12:15:00Z")
		})
	}
	t.Run("test_lint_issue_serialization", func(t *testing.T) {
		v := legacyDomainJSON(t, ProjectLintIssue{Severity: "error", Kind: "missing_directory", Message: "missing sources/ directory", Path: "sources/", Fixable: true})
		for key, want := range map[string]any{"severity": "error", "kind": "missing_directory", "message": "missing sources/ directory", "fixable": true, "fixed": false} {
			legacyConfigEqual(t, v[key], want)
		}
	})
	t.Run("test_lint_kind_serde", func(t *testing.T) {
		for _, kind := range []string{"stale_index_entry", "name_convention", "missing_manifest"} {
			legacyConfigEqual(t, legacyDomainJSON(t, ProjectLintIssue{Kind: kind})["kind"], kind)
		}
	})
}

func TestLegacyUnknownSourceKind(t *testing.T) {
	var source Source
	if err := yaml.Unmarshal([]byte("name: synthetic\ntype: file\nsource_kind: article_prompt\n"), &source); err != nil {
		t.Fatal(err)
	}
	legacyConfigPointer(t, source.SourceKind, "article_prompt")
	raw, err := yaml.Marshal(source)
	if err != nil || !strings.Contains(string(raw), "source_kind: article_prompt\n") {
		t.Fatalf("unknown source kind lost: %s %v", raw, err)
	}
	var again Source
	if err := yaml.Unmarshal(raw, &again); err != nil {
		t.Fatal(err)
	}
	legacyConfigPointer(t, again.SourceKind, "article_prompt")
}
