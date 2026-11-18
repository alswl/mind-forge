package store

import (
	"bytes"
	"testing"
)

func TestYAMLSectionSplicePreservesUnrelatedBytes(t *testing.T) {
	original := []byte("# header\r\nschema_version: '1'\r\nsources:\r\n- name: old\r\n\r\n# terms stay quoted\r\nterms:\r\n  x: 'unchanged'\r\nplugin: {value: 17}")
	document, err := ParseYAML(original)
	if err != nil {
		t.Fatal(err)
	}
	wantSection := []byte("sources:\r\n- name: old\r\n")
	got, exists := document.Section("sources")
	if !exists || !bytes.Equal(got, wantSection) {
		t.Fatalf("section span: %q", got)
	}
	replacement := []byte("sources:\r\n  - name: new\r\n")
	updated := document.ReplaceSection("sources", replacement)
	want := bytes.Replace(original, wantSection, replacement, 1)
	if !bytes.Equal(updated, want) || !bytes.Equal(document.Bytes(), original) {
		t.Fatalf("splice changed unrelated bytes or original: %q", updated)
	}
}

func TestYAMLSectionAppendAndTrailingComment(t *testing.T) {
	for _, original := range []string{"", "schema: '1'", "schema: '1'\n"} {
		document, err := ParseYAML([]byte(original))
		if err != nil {
			t.Fatal(err)
		}
		got := document.ReplaceSection("terms", []byte("terms: []\n"))
		want := original
		if want != "" && want[len(want)-1] != '\n' {
			want += "\n"
		}
		want += "terms: []\n"
		if string(got) != want {
			t.Fatalf("append: %q, want %q", got, want)
		}
	}
	document, err := ParseYAML([]byte("sources: []\n\n# footer"))
	if err != nil {
		t.Fatal(err)
	}
	if got := document.ReplaceSection("sources", []byte("sources: [a]\n")); string(got) != "sources: [a]\n\n# footer" {
		t.Fatalf("footer belongs outside replaced section: %q", got)
	}
}

func TestYAMLRejectsMalformedAndMultipleDocuments(t *testing.T) {
	for _, input := range []string{"[broken", "a: b\n---\nc: d\n", "a: [1, 2}"} {
		if _, err := ParseYAML([]byte(input)); err == nil {
			t.Errorf("accepted invalid single YAML document: %q", input)
		}
	}
}
