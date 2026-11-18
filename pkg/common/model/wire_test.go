package model

import (
	"encoding/json"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestManifestLegacyProjectsAndDefaults(t *testing.T) {
	var manifest Manifest
	err := yaml.Unmarshal([]byte("schema: '1'\nprojects:\n- alpha\n- name: beta\n  path: projects/beta\n  created_at: '2026-01-01T00:00:00Z'\n"), &manifest)
	if err != nil {
		t.Fatal(err)
	}
	if manifest.SchemaVersion != "1" || manifest.ProjectsDir != "projects" || len(manifest.Projects) != 2 {
		t.Fatalf("unexpected manifest: %#v", manifest)
	}
	if first := manifest.Projects[0]; first.Name != "alpha" || first.Path != "alpha" || first.CreatedAt != "" || first.ArchivedAt != nil {
		t.Fatalf("legacy path entry: %#v", first)
	}
	encoded, err := yaml.Marshal(manifest.Projects[1])
	if err != nil || !strings.Contains(string(encoded), "name: beta") || !strings.Contains(string(encoded), "path: projects/beta") {
		t.Fatalf("project must serialize as a mapping: %s (%v)", encoded, err)
	}
}

func TestDomainCollectionsKeepArrayAndObjectWireTypes(t *testing.T) {
	for _, item := range []struct {
		value           any
		arrays, objects []string
	}{
		{Mind{}, []string{"outputs"}, []string{"roles"}},
		{Source{Name: "example", Kind: "file"}, []string{"tags"}, nil},
		{Term{Term: "SDK"}, []string{"aliases", "tags", "corrections"}, nil},
		{Asset{Name: "example", Kind: "other"}, []string{"tags"}, nil},
	} {
		raw, err := json.Marshal(item.value)
		if err != nil {
			t.Fatal(err)
		}
		var fields map[string]json.RawMessage
		if err := json.Unmarshal(raw, &fields); err != nil {
			t.Fatal(err)
		}
		for _, name := range item.arrays {
			if string(fields[name]) != "[]" {
				t.Errorf("%T %s=%s, want []", item.value, name, fields[name])
			}
		}
		for _, name := range item.objects {
			if string(fields[name]) != "{}" {
				t.Errorf("%T %s=%s, want {}", item.value, name, fields[name])
			}
		}
	}
}

func TestManifestRejectsInvalidRequiredFields(t *testing.T) {
	for _, input := range []string{
		"projects: null\n", "projects_dir: null\n", "schema_version: null\n",
		"projects: [{name: alpha, path: alpha}]\n",
		"schema: '1'\nschema_version: '1'\n", "projects_dir: a\nprojects_dir: b\n",
	} {
		t.Run(input, func(t *testing.T) {
			var manifest Manifest
			if err := yaml.Unmarshal([]byte(input), &manifest); err == nil {
				t.Fatalf("accepted invalid manifest: %s", input)
			}
		})
	}
}

func TestManifestEmptyProjectsJSONIsArray(t *testing.T) {
	var manifest Manifest
	if err := yaml.Unmarshal([]byte("{}"), &manifest); err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(manifest)
	if err != nil || !strings.Contains(string(encoded), `"projects":[]`) {
		t.Fatalf("empty projects must be [], got %s (%v)", encoded, err)
	}
}

func TestIndexDictionaryNormalizationAndUnknownMetadata(t *testing.T) {
	input := "schema: '1'\nsources:\n  sources/a.md:\n    source_kind: extension-kind\n    retained: {enabled: false}\nfiles:\n  sources/b.md: {type: markdown}\nassets:\n  diagram: null\narticles:\n  short-note: {}\nplugins: {future: [false, null, '2026-01-01']}\n"
	var index Index
	if err := yaml.Unmarshal([]byte(input), &index); err != nil {
		t.Fatal(err)
	}
	if len(*index.Sources) != 2 || (*index.Sources)[0].Name != "sources/a.md" || *(*index.Sources)[1].Path != "sources/b.md" || (*index.Sources)[1].Kind != "file" {
		t.Fatalf("source/file normalization: %#v", index.Sources)
	}
	if *(*index.Sources)[0].SourceKind != "extension-kind" || (*index.Sources)[0].Extra.Get("retained") == nil {
		t.Fatalf("source unknown metadata lost: %#v", (*index.Sources)[0])
	}
	if asset := (*index.Assets)[0]; asset.Name != "diagram" || asset.Kind != "other" || asset.Size != 0 || len(asset.Tags) != 0 {
		t.Fatalf("asset defaults: %#v", asset)
	}
	if article := (*index.Articles)[0]; article.Title != "short note" || article.Path != "docs/short-note" || article.Type != "blog" || article.Status != "draft" {
		t.Fatalf("article defaults: %#v", article)
	}
	if len(index.Extra) != 1 || index.Extra[0].Key != "plugins" {
		t.Fatalf("unknown index metadata lost: %#v", index.Extra)
	}
}

func TestIndexAbsentNullEmptyAndLegacyOpaqueSections(t *testing.T) {
	var index Index
	if err := yaml.Unmarshal([]byte("sources: null\nassets: []\nprompts: active\n"), &index); err != nil {
		t.Fatal(err)
	}
	if index.Sources == nil || len(*index.Sources) != 0 || index.Assets == nil || index.Articles != nil || index.Prompts != nil {
		t.Fatalf("section presence changed: %#v", index)
	}
	encoded, err := json.Marshal(index)
	if err != nil || !strings.Contains(string(encoded), `"sources":[]`) || strings.Contains(string(encoded), `"articles"`) {
		t.Fatalf("wire section presence changed: %s (%v)", encoded, err)
	}
}

func TestIndexRejectsMalformedTypedEntries(t *testing.T) {
	for _, input := range []string{
		"sources: [{name: bad, type: alien}]\n", "assets: [{name: bad, size: -1}]\n",
		"assets: [{name: bad, type: unknown}]\n", "articles: [{}]\n",
	} {
		var index Index
		if err := yaml.Unmarshal([]byte(input), &index); err == nil {
			t.Errorf("accepted malformed typed entry: %s", input)
		}
	}
}

func TestIndexRejectsRecursiveUnknownMetadata(t *testing.T) {
	var index Index
	if err := yaml.Unmarshal([]byte("plugin: &loop {self: *loop}\n"), &index); err == nil {
		t.Fatal("recursive alias in unknown metadata must be rejected")
	}
}

func TestSourceMetadataWireKeepsUnknownFieldsAndDateStrings(t *testing.T) {
	var source Source
	input := "name: sample\nsource_kind: custom-channel\nfuture: {date: 2026-01-01, enabled: false}\nretained_scalar: 'value'\n"
	if err := yaml.Unmarshal([]byte(input), &source); err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(source)
	if err != nil || !strings.Contains(string(encoded), `"future":{"date":"2026-01-01","enabled":false}`) || !strings.Contains(string(encoded), `"retained_scalar":"value"`) || !strings.Contains(string(encoded), `"tags":[]`) {
		t.Fatalf("unknown metadata wire: %s (%v)", encoded, err)
	}
	if _, err := yaml.Marshal(source); err != nil {
		t.Fatal(err)
	}
}

func TestIndexAliasSectionsAreResolvedWithoutMutatingTheNode(t *testing.T) {
	var root yaml.Node
	input := "sources: &registered [{name: sample}]\nfiles: *registered\n"
	if err := yaml.Unmarshal([]byte(input), &root); err != nil {
		t.Fatal(err)
	}
	var first, second Index
	if err := root.Decode(&first); err != nil {
		t.Fatal(err)
	}
	if err := root.Decode(&second); err != nil {
		t.Fatal(err)
	}
	if first.Sources == nil || len(*first.Sources) != 2 || len(*second.Sources) != 2 {
		t.Fatalf("alias merge changed source node or dropped entries: %#v, %#v", first.Sources, second.Sources)
	}
}

func TestIndexTypedRecordsRejectMalformedValues(t *testing.T) {
	for _, input := range []string{
		"prompts: [{mode: unknown}]",
		"prompts: [{path: null}]",
		"thinking: [{article: null}]",
		"publish_records: [{path: docs/a.md, target_name: site}]",
		"publish_records: [{path: docs/a.md, target_name: site, status: unknown}]",
		"articles: [{title: example, template_origin: {template_name: daily}}]",
		"sources: [null]",
		"sources: [{name: example, tags: [null]}]",
		"terms: [null]",
		"terms: [{term: example, aliases: [null]}]",
		"terms: [{term: example, corrections: [null]}]",
		"assets: [{name: example, tags: [null]}]",
	} {
		t.Run(input, func(t *testing.T) {
			var index Index
			if err := yaml.Unmarshal([]byte(input), &index); err == nil {
				t.Fatalf("malformed typed record accepted: %s", input)
			}
		})
	}
	var index Index
	if err := yaml.Unmarshal([]byte("prompts: [{mode: decision-research}]\nthinking: [{}]\npublish_records: [{path: docs/a.md, target_name: site, status: archived}]\n"), &index); err != nil {
		t.Fatal(err)
	}
	if (*index.Prompts)[0].Path != "" || (*index.Thinking)[0].Path != "" {
		t.Fatal("projection defaults changed")
	}
}

func TestConfigAndManifestRejectNullTypedCollectionEntries(t *testing.T) {
	for _, input := range []string{
		"minds: {example: null}",
		"reserved_roots: [null]",
		"templates: {daily: null}",
		"build: {pipeline: [null]}",
		"build: {articles: {example: null}}",
		"source: {scan_paths: [null]}",
		"publish: {targets: [null]}",
	} {
		var config MindConfig
		if err := yaml.Unmarshal([]byte(input), &config); err == nil {
			t.Errorf("null typed entry accepted: %s", input)
		}
	}
	var manifest Manifest
	if err := yaml.Unmarshal([]byte("projects: [null]"), &manifest); err == nil {
		t.Error("null manifest project accepted")
	}
}
