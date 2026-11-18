package model

import (
	"reflect"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestLegacyIndexModels(t *testing.T) {
	for _, test := range []struct{ name, input string }{
		{"test_deserialize_dictionary_sources", "\nschema_version: '1'\nsources:\n  docs/api-spec.md:\n    name: API Spec\n    type: file\n    path: docs/api-spec.md\n  docs/arch.md:\n    name: Architecture\n    type: file\n    path: docs/arch.md\n"},
		{"test_deserialize_files_dictionary_as_sources", "\nschema_version: '1'\nfiles:\n  docs/api-spec.md:\n    type: markdown\n    description: API reference\n"},
		{"test_invalid_known_index_entry_returns_error", "\nschema_version: '1'\nsources:\n  docs/api-spec.md:\n    type: not_a_file_kind\n"},
		{"test_deserialize_dictionary_assets", "\nschema_version: '1'\nassets:\n  logo.png:\n    name: logo.png\n    type: image\n    path: assets/logo.png\n  banner.jpg:\n    name: banner.jpg\n    type: image\n    path: assets/banner.jpg\n"},
		{"test_deserialize_dictionary_articles", "\nschema_version: '1'\narticles:\n  getting-started:\n    title: Getting Started\n    article_path: docs/getting-started.md\n  advanced-guide:\n    title: Advanced Guide\n    article_path: docs/advanced-guide.md\n"},
		{"test_deserialize_null_entries", "\nschema_version: '1'\nsources:\n  null-source:\narticles:\n  null-article:\n"},
		{"test_preserve_extra_fields", "\nschema_version: '1'\nproject: my-project\nupdated: 2026-05-01\ndocs: active\n"},
		{"test_legacy_opaque_prompts_thinking_scalar_does_not_error", "\nschema_version: '1'\nupdated: 2026-05-01\nprompts: active\nthinking: active\n"},
		{"extra_fields_round_trip_in_original_order_deterministically", "schema_version: '1'\nalpha: 1\nzulu: 2\nmiddle: 3\n"},
		{"test_deserialize_dictionary_prompts", "\nschema_version: '1'\nprompts:\n  my-post:\n    path: prompts/my-post.md\n    article: docs/my-post.md\n    mode: research\n    updated_at: '2026-07-12T09:00:00Z'\n  orphaned:\n    article: docs/does-not-exist.md\n"},
		{"test_deserialize_dictionary_thinking", "\nschema_version: '1'\nthinking:\n  my-post:\n    path: thinking/my-post.md\n    article: docs/my-post.md\n    updated_at: '2026-07-12T09:00:00Z'\n"},
		{"test_deserialize_list_format_prompts_and_thinking", "\nschema_version: '1'\nprompts:\n  - path: prompts/my-post.md\n    article: docs/my-post.md\n    updated_at: '2026-07-12T09:00:00Z'\nthinking:\n  - path: thinking/my-post.md\n    article: docs/my-post.md\n    updated_at: '2026-07-12T09:00:00Z'\n"},
		{"test_invalid_prompt_mode_returns_error", "\nschema_version: '1'\nprompts:\n  my-post:\n    path: prompts/my-post.md\n    article: docs/my-post.md\n    mode: not_a_mode\n"},
		{"test_schema_alias", "\nschema: '1'\n"},
		{"test_list_format_still_works", "\nschema_version: '1'\nsources:\n  - name: Source 1\n    type: file\n    path: docs/source1.md\n"},
		{"test_parse_list_format_assets_with_type_other_succeeds", "schema_version: '1'\nassets:\n  - name: notes.pdf\n    type: other\n    path: assets/notes.pdf\n    size: 4096\n    hash: '77889900'\n    tags: []\n    added_at: '2026-05-07T21:00:00Z'\n  - name: banner.jpg\n    type: image\n    path: assets/banner.jpg\n    size: 24576\n    hash: aabb1122\n    tags: [hero]\n    added_at: '2026-05-07T18:00:00Z'\n"},
	} {
		t.Run(test.name, func(t *testing.T) { checkLegacyIndex(t, test.name, test.input) })
	}
}

func checkLegacyIndex(t *testing.T, name, input string) {
	t.Helper()
	var index Index
	err := yaml.Unmarshal([]byte(input), &index)
	if name == "test_invalid_known_index_entry_returns_error" || name == "test_invalid_prompt_mode_returns_error" {
		part := "invalid sources[0]"
		if name == "test_invalid_prompt_mode_returns_error" {
			part = "invalid prompts[0]"
		}
		if err == nil || !strings.Contains(err.Error(), part) {
			t.Fatalf("invalid known entry not diagnosed: %v", err)
		}
		return
	}
	if err != nil {
		t.Fatal(err)
	}
	switch name {
	case "test_deserialize_dictionary_sources":
		if index.SchemaVersion != "1" || index.Sources == nil || len(*index.Sources) != 2 {
			t.Fatalf("dictionary sources: %+v", index)
		}
	case "test_deserialize_files_dictionary_as_sources":
		if index.Sources == nil || len(*index.Sources) != 1 {
			t.Fatal("files alias did not produce one source")
		}
		item := (*index.Sources)[0]
		if item.Name != "docs/api-spec.md" || item.Path == nil || *item.Path != "docs/api-spec.md" {
			t.Fatalf("files alias identity lost: %+v", item)
		}
	case "test_deserialize_dictionary_assets", "test_parse_list_format_assets_with_type_other_succeeds":
		if index.Assets == nil || len(*index.Assets) != 2 {
			t.Fatal("expected two assets")
		}
	case "test_deserialize_dictionary_articles":
		if index.Articles == nil || len(*index.Articles) != 2 {
			t.Fatal("expected two articles")
		}
	case "test_deserialize_null_entries":
		if index.Sources == nil || len(*index.Sources) != 1 || index.Articles == nil || len(*index.Articles) != 1 {
			t.Fatal("null entries not normalized into entries")
		}
	case "test_preserve_extra_fields":
		for _, key := range []string{"project", "updated", "docs"} {
			found := false
			for _, field := range index.Extra {
				if field.Key == key {
					found = true
				}
			}
			if !found {
				t.Fatalf("unknown key %q lost", key)
			}
		}
	case "test_legacy_opaque_prompts_thinking_scalar_does_not_error":
		if index.Prompts != nil || index.Thinking != nil {
			t.Fatal("opaque role scalars did not degrade to absent projection")
		}
		updated := false
		for _, field := range index.Extra {
			if field.Key == "updated" {
				updated = true
			}
			if field.Key == "prompts" || field.Key == "thinking" {
				t.Fatal("known role key leaked into extra")
			}
		}
		if !updated {
			t.Fatal("unrelated updated key lost")
		}
	case "extra_fields_round_trip_in_original_order_deterministically":
		renders := map[string]bool{}
		for i := 0; i < 20; i++ {
			var again Index
			if err := yaml.Unmarshal([]byte(input), &again); err != nil {
				t.Fatal(err)
			}
			keys := []string{}
			for _, field := range again.Extra {
				keys = append(keys, field.Key)
			}
			if !reflect.DeepEqual(keys, []string{"alpha", "zulu", "middle"}) {
				t.Fatalf("extra field order = %v", keys)
			}
			raw, err := yaml.Marshal(again)
			if err != nil {
				t.Fatal(err)
			}
			renders[string(raw)] = true
		}
		if len(renders) != 1 {
			t.Fatalf("same index produced %d renderings", len(renders))
		}
	case "test_deserialize_dictionary_prompts":
		if index.Prompts == nil || len(*index.Prompts) != 2 {
			t.Fatal("expected two prompts")
		}
		bound, orphan := false, false
		for _, item := range *index.Prompts {
			if item.Path == "prompts/my-post.md" {
				bound = true
				if item.Article != "docs/my-post.md" || item.Mode == nil || *item.Mode != "research" {
					t.Fatalf("bound prompt changed: %+v", item)
				}
			}
			if item.Path == "prompts/orphaned.md" {
				orphan = true
				if item.Article != "docs/does-not-exist.md" {
					t.Fatalf("orphan binding changed: %+v", item)
				}
			}
		}
		if !bound || !orphan {
			t.Fatal("prompt path inference lost")
		}
	case "test_deserialize_dictionary_thinking":
		if index.Thinking == nil || len(*index.Thinking) != 1 {
			t.Fatal("expected one thinking entry")
		}
		item := (*index.Thinking)[0]
		if item.Path != "thinking/my-post.md" || item.Article != "docs/my-post.md" {
			t.Fatalf("thinking binding changed: %+v", item)
		}
	case "test_deserialize_list_format_prompts_and_thinking":
		if index.Prompts == nil || len(*index.Prompts) != 1 || index.Thinking == nil || len(*index.Thinking) != 1 {
			t.Fatal("list role entries lost")
		}
	case "test_schema_alias":
		if index.SchemaVersion != "1" {
			t.Fatal(index.SchemaVersion)
		}
	case "test_list_format_still_works":
		if index.Sources == nil || len(*index.Sources) != 1 {
			t.Fatal("list source lost")
		}
	default:
		t.Fatalf("unhandled frozen index case %q", name)
	}
}
