package model

import (
	"go.yaml.in/yaml/v3"
	"reflect"
	"strings"
	"testing"
)

func legacyConfigEqual(t *testing.T, got, want any) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("config value = %#v, want %#v", got, want)
	}
}
func legacyConfigPointer[T any](t *testing.T, got *T, want T) {
	t.Helper()
	if got == nil {
		t.Fatal("required optional field absent")
	}
	legacyConfigEqual(t, *got, want)
}
func legacyConfigRoundTrip(t *testing.T, c MindConfig) MindConfig {
	t.Helper()
	raw, err := yaml.Marshal(c)
	if err != nil {
		t.Fatal(err)
	}
	var again MindConfig
	if err = yaml.Unmarshal(raw, &again); err != nil {
		t.Fatal(err)
	}
	return again
}

func TestLegacyMindConfig(t *testing.T) {
	for _, test := range []struct{ name, input string }{
		{"test_empty_mind_yaml_defaults", "schema_version: '1'\n"},
		{"test_schema_alias", "schema: '1'\n"},
		{"test_missing_schema_version_defaults_to_1", "name: test\n"},
		{"test_null_build_output_dir_defaults_to_outputs", "build:\n  output_dir:\n"},
		{"test_layout_canonical_populates_layout_field", "layout:\n  articles: notes\n  sources: refs\n  assets: media\n"},
		{"test_layout_docs_compat_alias", "layout:\n  docs: notes\n  sources: refs\n  assets: media\n"},
		{"test_paths_still_readable", "paths:\n  docs: notes\n  sources: refs\n  assets: media\n"},
		{"test_top_level_name_and_description", "\nschema_version: '1'\nname: My Project\ndescription: A test project\ncreated: 2026-01-01\nupdated: 2026-05-10\n"},
		{"test_top_level_articles_with_null_value", "\nschema_version: '1'\narticles:\n  weekly-summary:\n    type: blog\n  null-entry:\n"},
		{"test_local_publish_target_with_direct_path", "\nschema_version: '1'\npublish:\n  targets:\n    - name: local-pdf\n      type: local\n      path: ./output\n      prefix: reports-\n"},
		{"test_yuque_cc_publish_target", "\nschema_version: '1'\npublish:\n  targets:\n    - name: yuque-tech\n      type: yuque_cc\n      book_slug: tech-blog\n      namespace: engineering\n"},
		{"test_unknown_top_level_fields_ignored", "\nschema_version: '1'\nunknown_field: value\nanother_unknown: 42\n"},
		{"build_new_options_default_off_and_pipeline_empty", "schema_version: '1'\n"},
		{"build_pipeline_rule_deserializes_dark_default", "build:\n  pipeline:\n    - name: svg-to-png\n      input_extension: svg\n      output_extension: png\n      command: cp {input} {output}\n"},
		{"templates_round_trip_generated_mode", "\nschema_version: '1'\ntemplates:\n  daily_report:\n    pattern: \"outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md\"\n    mode: generated\n"},
		{"templates_preserve_unknown_mode_as_other", "\nschema_version: '1'\ntemplates:\n  custom:\n    pattern: \"outputs/{date:YYYY-MM-DD}.md\"\n    mode: custom_mode\n"},
		{"templates_preserve_extra_fields_like_cadence", "\nschema_version: '1'\ntemplates:\n  weekly:\n    pattern: \"outputs/{date:YYYY-MM-DD}.md\"\n    mode: generated\n    cadence: weekly\n"},
		{"templates_absent_deserializes_as_none", "\nschema_version: '1'\nproject:\n  name: test\n"},
		{"plugins_missing_yields_none", "schema_version: '1'\n"},
		{"plugins_absent_but_other_fields_present", "schema_version: '1'\nproject:\n  name: test\n"},
		{"typora_front_matter_enabled_true", "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: true\n"},
		{"typora_front_matter_enabled_false", "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: false\n"},
		{"typora_front_matter_missing_enabled", "schema_version: '1'\nplugins:\n  typora-front-matter: {}\n"},
		{"plugins_unknown_plugin_key_preserved", "schema_version: '1'\nplugins:\n  some-other-plugin:\n    key: value\n"},
		{"typora_front_matter_unknown_field_preserved", "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: true\n    custom: keep-me\n"},
		{"plugins_empty_block_yields_present", "schema_version: '1'\nplugins: {}\n"},
		{"typora_front_matter_scalar_is_invalid", "schema_version: '1'\nplugins:\n  typora-front-matter: false\n"},
		{"typora_enabled_string_is_invalid", "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: \"yes\"\n"},
		{"typora_enabled_number_is_invalid", "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: 1\n"},
		{"plugin_unknown_round_trips_through_mind_config", "schema_version: '1'\nplugins:\n  some-other-plugin:\n    key: value\n"},
	} {
		t.Run(test.name, func(t *testing.T) { checkLegacyMindConfig(t, test.name, test.input) })
	}
}

func checkLegacyMindConfig(t *testing.T, name, input string) {
	t.Helper()
	var c MindConfig
	err := yaml.Unmarshal([]byte(input), &c)
	switch name {
	case "typora_front_matter_scalar_is_invalid", "typora_enabled_string_is_invalid", "typora_enabled_number_is_invalid":
		if err == nil {
			t.Fatal("invalid plugin configuration accepted")
		}
		return
	}
	if err != nil {
		t.Fatal(err)
	}
	switch name {
	case "test_empty_mind_yaml_defaults":
		legacyConfigEqual(t, c.SchemaVersion, "1")
		if c.Project != nil || c.Name != nil {
			t.Fatal("empty config populated project/name")
		}
	case "test_schema_alias", "test_missing_schema_version_defaults_to_1", "test_unknown_top_level_fields_ignored":
		legacyConfigEqual(t, c.SchemaVersion, "1")
	case "test_null_build_output_dir_defaults_to_outputs":
		legacyConfigEqual(t, c.Build.OutputDir, "outputs")
	case "test_layout_canonical_populates_layout_field", "test_layout_docs_compat_alias":
		if c.Layout == nil {
			t.Fatal("layout absent")
		}
		legacyConfigPointer(t, c.Layout.Articles, "notes")
		legacyConfigPointer(t, c.Layout.Sources, "refs")
		legacyConfigPointer(t, c.Layout.Assets, "media")
	case "test_paths_still_readable":
		legacyConfigEqual(t, c.Paths.Docs, "notes")
		legacyConfigEqual(t, c.Paths.Sources, "refs")
		legacyConfigEqual(t, c.Paths.Assets, "media")
		if c.Layout != nil {
			t.Fatal("legacy paths populated optional layout")
		}
	case "test_top_level_name_and_description":
		legacyConfigPointer(t, c.Name, "My Project")
		legacyConfigPointer(t, c.Description, "A test project")
		legacyConfigPointer(t, c.Created, "2026-01-01")
		legacyConfigPointer(t, c.Updated, "2026-05-10")
	case "test_top_level_articles_with_null_value":
		if c.Articles == nil {
			t.Fatal("articles absent")
		}
	case "test_local_publish_target_with_direct_path", "test_yuque_cc_publish_target":
		if c.Publish.Targets == nil || len(*c.Publish.Targets) != 1 {
			t.Fatal("expected one publish target")
		}
		target := (*c.Publish.Targets)[0]
		if name == "test_local_publish_target_with_direct_path" {
			legacyConfigEqual(t, target.Name, "local-pdf")
			legacyConfigEqual(t, target.Type, "local")
			legacyConfigPointer(t, target.Path, "./output")
			legacyConfigPointer(t, target.Prefix, "reports-")
		} else {
			legacyConfigEqual(t, target.Type, "yuque_cc")
			legacyConfigPointer(t, target.BookSlug, "tech-blog")
			legacyConfigPointer(t, target.Namespace, "engineering")
		}
	case "build_new_options_default_off_and_pipeline_empty":
		if c.Build.StripFirstH1 || len(c.Build.Pipeline) != 0 {
			t.Fatal("new build options unexpectedly active")
		}
	case "build_pipeline_rule_deserializes_dark_default":
		if len(c.Build.Pipeline) != 1 || c.Build.Pipeline[0].Dark {
			t.Fatal("pipeline dark default changed")
		}
	case "templates_round_trip_generated_mode":
		if c.Templates == nil {
			t.Fatal("templates absent")
		}
		item, ok := (*c.Templates)["daily_report"]
		if !ok {
			t.Fatal("daily_report absent")
		}
		legacyConfigEqual(t, item.Pattern, "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md")
		legacyConfigEqual(t, item.Mode, "generated")
		again := legacyConfigRoundTrip(t, c)
		if again.Templates == nil {
			t.Fatal("templates lost on round trip")
		}
		next, ok := (*again.Templates)["daily_report"]
		if !ok {
			t.Fatal("daily_report lost on round trip")
		}
		legacyConfigEqual(t, next.Pattern, item.Pattern)
		legacyConfigEqual(t, next.Mode, "generated")
	case "templates_preserve_unknown_mode_as_other":
		if c.Templates == nil {
			t.Fatal("templates absent")
		}
		item, ok := (*c.Templates)["custom"]
		if !ok {
			t.Fatal("custom template absent")
		}
		legacyConfigEqual(t, item.Mode, "custom_mode")
	case "templates_preserve_extra_fields_like_cadence":
		if c.Templates == nil {
			t.Fatal("templates absent")
		}
		item, ok := (*c.Templates)["weekly"]
		if !ok || item.Extra.Get("cadence") == nil {
			t.Fatal("cadence metadata absent")
		}
		raw, err := yaml.Marshal(c)
		if err != nil || !strings.Contains(string(raw), "cadence:") {
			t.Fatalf("cadence not serialized: %s %v", raw, err)
		}
	case "templates_absent_deserializes_as_none":
		if c.Templates != nil {
			t.Fatal("absent templates populated")
		}
	case "plugins_missing_yields_none", "plugins_absent_but_other_fields_present":
		if c.Plugins != nil {
			t.Fatal("absent plugins populated")
		}
	case "plugins_empty_block_yields_present":
		if c.Plugins == nil || c.Plugins.Typora != nil {
			t.Fatal("empty plugins lost presence or populated Typora")
		}
	case "typora_front_matter_enabled_true", "typora_front_matter_enabled_false", "typora_front_matter_missing_enabled", "typora_front_matter_unknown_field_preserved":
		if c.Plugins == nil || c.Plugins.Typora == nil {
			t.Fatal("Typora plugin absent")
		}
		if name == "typora_front_matter_missing_enabled" {
			if c.Plugins.Typora.Enabled != nil {
				t.Fatal("absent enabled populated")
			}
		} else {
			legacyConfigPointer(t, c.Plugins.Typora.Enabled, name != "typora_front_matter_enabled_false")
		}
		if name == "typora_front_matter_unknown_field_preserved" {
			raw, err := yaml.Marshal(c.Plugins)
			if err != nil || !strings.Contains(string(raw), "custom: keep-me") {
				t.Fatalf("unknown Typora field lost: %s %v", raw, err)
			}
		}
	case "plugins_unknown_plugin_key_preserved":
		if c.Plugins == nil || c.Plugins.Typora != nil {
			t.Fatal("unknown plugin presence changed")
		}
		raw, err := yaml.Marshal(c.Plugins)
		if err != nil || !strings.Contains(string(raw), "some-other-plugin") {
			t.Fatalf("unknown plugin lost: %s %v", raw, err)
		}
	case "plugin_unknown_round_trips_through_mind_config":
		raw, err := yaml.Marshal(c)
		if err != nil || !strings.Contains(string(raw), "some-other-plugin") {
			t.Fatalf("unknown plugin lost through config: %s %v", raw, err)
		}
	default:
		t.Fatalf("unhandled frozen config case %q", name)
	}
}

func TestLegacyPublishTargetTypes(t *testing.T) {
	for _, test := range []struct{ name, kind string }{
		{"target_type_local_serializes_snake_case", "local"},
		{"target_type_yuque_prompt_uses_kebab_case", "yuque-prompt"},
		{"target_type_yuque_serializes_snake_case", "yuque"},
		{"target_type_github_pages_keeps_snake_case", "github_pages"},
		{"target_type_custom_serializes_snake_case", "custom"},
	} {
		t.Run(test.name, func(t *testing.T) {
			raw, err := yaml.Marshal(PublishTarget{Name: "x", Type: test.kind, Enabled: true})
			if err != nil || !strings.Contains(string(raw), "type: "+test.kind) {
				t.Fatalf("target spelling lost: %s %v", raw, err)
			}
		})
	}
	for _, test := range []struct{ name, kind string }{
		{"target_type_yuque_prompt_round_trips", "yuque-prompt"},
		{"target_type_github_pages_round_trips_snake_case", "github_pages"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var target PublishTarget
			if err := yaml.Unmarshal([]byte("name: x\ntype: "+test.kind+"\nenabled: true\n"), &target); err != nil {
				t.Fatal(err)
			}
			legacyConfigEqual(t, target.Type, test.kind)
		})
	}
}

func TestConfigBooleanFieldsRequireBooleanScalars(t *testing.T) {
	for _, record := range []struct{ name, prefix, suffix string }{
		{"plugin", "plugins: {typora-front-matter: {enabled: ", "}}"},
		{"term", "term: {enabled: ", "}"},
		{"build", "build: {strip_first_h1: ", "}"},
		{"pipeline", "build: {pipeline: [{name: x, input_extension: svg, output_extension: png, command: copy, dark: ", "}]}"},
		{"publish", "publish: {targets: [{name: x, type: local, enabled: ", "}]}"},
	} {
		t.Run(record.name, func(t *testing.T) {
			for _, value := range []string{`"yes"`, `"no"`, `"true"`, `"false"`, `"on"`, `"off"`, "yes", "off", "1", "[]", "{}"} {
				var c MindConfig
				if err := yaml.Unmarshal([]byte(record.prefix+value+record.suffix), &c); err == nil {
					t.Errorf("non-boolean scalar/shape %s was accepted", value)
				}
			}
			for _, value := range []string{"true", "false"} {
				var c MindConfig
				if err := yaml.Unmarshal([]byte(record.prefix+value+record.suffix), &c); err != nil {
					t.Errorf("boolean %s rejected: %v", value, err)
				}
			}
		})
	}
	for _, test := range []struct {
		input string
		fails bool
	}{
		{"value: &flag true\nplugins: {typora-front-matter: {enabled: *flag}}", false},
		{"value: &flag \"yes\"\nplugins: {typora-front-matter: {enabled: *flag}}", true},
		{"plugins: {typora-front-matter: {enabled: null, extra: \"yes\"}}", false},
	} {
		var c MindConfig
		err := yaml.Unmarshal([]byte(test.input), &c)
		if (err != nil) != test.fails {
			t.Fatalf("alias/null boolean decoding: %q: %v", test.input, err)
		}
		if err == nil && strings.Contains(test.input, "enabled: null") {
			if c.Plugins == nil || c.Plugins.Typora == nil || c.Plugins.Typora.Enabled != nil || c.Plugins.Typora.Extra.Get("extra") == nil {
				t.Fatal("optional null or unknown metadata changed")
			}
		}
	}
}
