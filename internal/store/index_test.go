package store

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestIndexReadAndTermOnlyWriteProtectUnrelatedSections(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	original := []byte("# retained\r\nschema: '1'\r\nsources:\r\n  sources/a.md: {name: a, retained: 'value'}\r\n\r\n# extension owns these bytes\r\nextension: {quoted: 'yes', date: 2026-01-01}\r\nterms:\r\n- term: API\r\n  aliases: []\r\n")
	if err := os.WriteFile(path, original, 0o600); err != nil {
		t.Fatal(err)
	}
	index, err := ReadIndex(path, true)
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(path); !bytes.Equal(got, original) {
		t.Fatal("read changed stored bytes")
	}
	updated := index.Value
	terms := []model.Term{{Term: "SDK", Aliases: []string{}, Tags: []string{}, Corrections: []model.Correction{}}}
	updated.Terms = &terms
	rendered, err := index.Render(updated)
	if err != nil {
		t.Fatal(err)
	}
	prefix := original[:bytes.Index(original, []byte("terms:"))]
	if !bytes.HasPrefix(rendered, prefix) || !bytes.Contains(rendered, []byte("SDK")) {
		t.Fatalf("unrelated bytes changed: %q", rendered)
	}
	if err := index.Write(path, updated, true); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(path); !bytes.Equal(got, original) {
		t.Fatal("dry-run wrote index")
	}
	if err := index.Write(path, updated, false); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(path); !bytes.Equal(got, rendered) {
		t.Fatal("published index differs from rendering")
	}
}

func TestIndexDuplicateRecoveryIsReadOnlyAndKeepsLastOccurrence(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	raw := []byte("schema: '1'\nterms:\n- term: old\nterms:\n- term: new\n  description: old\n  description: final\n")
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatal(err)
	}
	index, err := ReadIndex(path, true)
	if err != nil || len(index.Warnings) == 0 || (*index.Value.Terms)[0].Term != "new" || *(*index.Value.Terms)[0].Description != "final" {
		t.Fatalf("duplicate recovery: %#v (%v)", index, err)
	}
	if actual, _ := os.ReadFile(path); !bytes.Equal(actual, raw) {
		t.Fatal("duplicate recovery rewrote index")
	}
	updated := index.Value
	terms := []model.Term{{Term: "replacement", Tags: []string{}, Aliases: []string{}, Corrections: []model.Correction{}}}
	updated.Terms = &terms
	rendered, err := index.Render(updated)
	if err != nil || strings.Count(string(rendered), "terms:") != 1 || !strings.Contains(string(rendered), "replacement") {
		t.Fatalf("changed duplicate section still shadows new value: %s (%v)", rendered, err)
	}
}

func TestIndexMissingStrictLenientAndSchema(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	index, err := ReadIndex(path, true)
	if err != nil || index.Value.SchemaVersion != "1" || index.Value.Terms != nil {
		t.Fatalf("missing index default: %#v (%v)", index, err)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatal("missing index was created by reading")
	}
	invalid := []byte("schema: '1'\nterms:\n- term: SDK\n  corrections: [{original: '-sdk', correct: SDK}]\n")
	if err := os.WriteFile(path, invalid, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := ReadIndex(path, true); err == nil {
		t.Fatal("strict loader accepted an invalid standalone ASCII edge")
	}
	if _, err := ReadIndex(path, false); err != nil {
		t.Fatalf("repair loader must retain invalid corrections: %v", err)
	}
	if err := os.WriteFile(path, []byte("schema: '2'\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err = ReadIndex(path, false)
	var typed *output.Error
	if !errors.As(err, &typed) || typed.Kind != "incompatible_schema" || typed.ExitCode() != 2 {
		t.Fatalf("schema rejection: %v", err)
	}
	if typed.Hint == nil || *typed.Hint != "use a Mind Forge version that supports this schema" {
		t.Fatalf("schema hint = %v", typed.Hint)
	}
}

func TestIndexTermWritePreservesYAMLMetadataThatCannotSerializeToJSON(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	raw := []byte("sources:\n  a: {name: example, extension: {[a, b]: c}}\nterms: []\n")
	if err := os.WriteFile(path, raw, 0o600); err != nil {
		t.Fatal(err)
	}
	index, err := ReadIndex(path, true)
	if err != nil {
		t.Fatal(err)
	}
	updated := index.Value
	terms := []model.Term{{Term: "SDK"}}
	updated.Terms = &terms
	result, err := index.Render(updated)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasPrefix(result, raw[:bytes.Index(raw, []byte("terms:"))]) {
		t.Fatalf("opaque YAML metadata changed: %s", result)
	}
}

func TestIndexSectionUpdatePreservesDependentAliasValue(t *testing.T) {
	raw := []byte("schema: '1'\nassets: &original\n  missing: {name: missing, path: assets/missing.png, type: image}\ncustom: *original\n")
	document, err := DecodeIndex(raw, "mind-index.yaml", false)
	if err != nil {
		t.Fatal(err)
	}
	value := document.Value
	empty := []model.Asset{}
	value.Assets = &empty
	updated, err := document.Render(value)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeIndex(updated, "mind-index.yaml", false)
	if err != nil {
		t.Fatalf("dangling alias after section removal: %s %v", updated, err)
	}
	if len(decoded.Value.Extra) != 1 {
		t.Fatal("lost custom authority")
	}
}

func TestIndexUpdatesFlowAndQuotedSectionsWithoutDuplicates(t *testing.T) {
	for _, raw := range []string{"{schema: '1', assets: {missing: {name: missing, type: image}}, custom: {note: retained}}\n", "schema: '1'\n'assets':\n  missing: {name: missing, type: image}\ncustom: {note: retained}\n"} {
		document, err := DecodeIndex([]byte(raw), "mind-index.yaml", false)
		if err != nil {
			t.Fatal(err)
		}
		updated := document.Value
		empty := []model.Asset{}
		updated.Assets = &empty
		rendered, err := document.Render(updated)
		if err != nil {
			t.Fatal(err)
		}
		restored, err := DecodeIndex(rendered, "mind-index.yaml", false)
		if err != nil || restored.Value.Assets != nil || len(restored.Value.Extra) != 1 {
			t.Fatalf("%s %#v %v", rendered, restored, err)
		}
	}
}
