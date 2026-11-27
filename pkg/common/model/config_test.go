package model

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
	"go.yaml.in/yaml/v3"
)

func TestConfigReadDefaultsDifferFromExplicitDefaultConfig(t *testing.T) {
	var config MindConfig
	if err := yaml.Unmarshal([]byte("schema: '1'\nbuild: {output_dir: null}\n"), &config); err != nil {
		t.Fatal(err)
	}
	if config.SchemaVersion != "1" || config.Build.OutputDir != "outputs" || config.Build.Format != "md" || !config.Term.Enabled || config.Layout != nil || config.Plugins != nil {
		t.Fatalf("read defaults: %#v", config)
	}
	defaults := DefaultMindConfig()
	if defaults.Layout == nil || *defaults.Layout.Articles != "docs" || defaults.Plugins == nil || defaults.Plugins.Typora == nil || !*defaults.Plugins.Typora.Enabled {
		t.Fatalf("explicit defaults: %#v", defaults)
	}
	encoded, err := json.Marshal(config)
	if err != nil || !strings.Contains(string(encoded), `"merge_order":[]`) || !strings.Contains(string(encoded), `"articles":{}`) || strings.Contains(string(encoded), `"paths"`) {
		t.Fatalf("default wire shapes: %s (%v)", encoded, err)
	}
}

func TestConfigPreservesPluginAndTemplateMetadata(t *testing.T) {
	input := "content_layout: bundle\nminds: {brief: {title: 'Brief', default_output: summary}}\nlayout: {docs: notes, output_dir: public}\nplugins:\n  typora-front-matter: {enabled: false, future: [1, 2]}\n  custom: {date: 2026-01-01, switch: false}\ntemplates:\n  weekly: {pattern: 'weekly/{date:YYYY-MM-DD}', mode: generated, cadence: weekly}\npublish:\n  targets: [{name: local, type: local, path: public}]\n"
	var config MindConfig
	if err := yaml.Unmarshal([]byte(input), &config); err != nil {
		t.Fatal(err)
	}
	if *config.ContentLayout != "bundle" || *config.Layout.Articles != "notes" || *config.Layout.BuildOutput != "public" || *config.Plugins.Typora.Enabled {
		t.Fatalf("config normalization: %#v", config)
	}
	encoded, err := json.Marshal(config)
	if err != nil || !strings.Contains(string(encoded), `"custom":{"date":"2026-01-01","switch":false}`) || !strings.Contains(string(encoded), `"cadence":"weekly"`) || !strings.Contains(string(encoded), `"future":[1,2]`) {
		t.Fatalf("metadata lost: %s (%v)", encoded, err)
	}
	if !(*config.Publish.Targets)[0].Enabled {
		t.Fatal("publish target defaults to enabled")
	}
	encoded, err = yaml.Marshal(config)
	if err != nil || strings.HasPrefix(string(encoded), "paths:") || strings.Contains(string(encoded), "\npaths:") {
		t.Fatalf("config YAML: %s (%v)", encoded, err)
	}
	var again MindConfig
	if err := yaml.Unmarshal(encoded, &again); err != nil || *again.Plugins.Typora.Enabled || again.Plugins.Extra.Get("custom") == nil {
		t.Fatalf("metadata round trip failed: %#v (%v)", again, err)
	}
}

func TestConfigRoundTripsHeldBackTermPairs(t *testing.T) {
	input := []byte("term: {held_back_ignore: [{original: 机器, correct: 装置}]}")
	want := []HeldBackTermPair{{Original: "机器", Correct: "装置"}}
	var config MindConfig
	require.NoError(t, yaml.Unmarshal(input, &config))
	require.True(t, config.Term.Enabled)
	require.Equal(t, want, config.Term.HeldBackIgnore)

	encoded, err := yaml.Marshal(config)
	require.NoError(t, err)
	var again MindConfig
	require.NoError(t, yaml.Unmarshal(encoded, &again))
	require.Equal(t, want, again.Term.HeldBackIgnore)
}

func TestConfigRejectsRequiredNullAndInvalidEnums(t *testing.T) {
	for _, input := range []string{
		"schema_version: null", "build: null", "source: {types: null}", "term: {enabled: null}",
		"term: {held_back_ignore: null}", "term: {held_back_ignore: [{original: foo}]}",
		"term: {held_back_ignore: [{original: '', correct: bar}]}",
		"build: {merge_order: null}", "content_layout: guessed", "layout: {docs: a, articles: b}",
		"publish: {targets: [{name: p, type: guessed}]}", "publish: {targets: [{name: p}]}",
		"templates: {x: {pattern: anything}}", "project: {description: missing-name}",
	} {
		var config MindConfig
		if err := yaml.Unmarshal([]byte(input), &config); err == nil {
			t.Errorf("accepted invalid config: %s", input)
		}
	}
}
