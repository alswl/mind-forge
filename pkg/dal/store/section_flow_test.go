package store

import (
	"bytes"
	"reflect"
	"testing"
)

func TestProtectedSectionAddsToFlowRoot(t *testing.T) {
	for _, fixture := range []struct{ name, raw string }{
		{"json", `{"schema_version":"1","terms":[],"author_marker":{"synthetic":"retained"}}`},
		{"flow-yaml", "{schema_version: '1', terms: [], author_marker: {synthetic: retained}}\n"},
		{"multiline", "# Author header\n{schema_version: '1',\n terms: [], author_marker: {synthetic: retained}}\n"},
		{"crlf", "{schema_version: '1', terms: [], author_marker: {synthetic: retained}}\r\n"},
	} {
		raw := fixture.raw
		for _, section := range []string{"sources", "assets", "articles"} {
			t.Run(section+"/"+fixture.name, func(t *testing.T) {
				document, err := ParseYAML([]byte(raw))
				if err != nil {
					t.Fatal(err)
				}
				var expected map[string]any
				if err := document.DecodeMapping(&expected); err != nil {
					t.Fatal(err)
				}
				expected[section] = []any{map[string]any{"name": "synthetic", "path": "synthetic.md"}}
				updated, err := document.ReplaceProtectedSection(section, []byte(section+":\n  - name: synthetic\n    path: synthetic.md\n"))
				if err != nil {
					t.Fatal(err)
				}
				reopened, err := ParseYAML(updated)
				if err != nil {
					t.Fatalf("successful section update produced unreadable YAML: %v\n%s", err, updated)
				}
				var actual map[string]any
				if err := reopened.DecodeMapping(&actual); err != nil {
					t.Fatal(err)
				}
				if !reflect.DeepEqual(actual, expected) {
					t.Fatalf("updated mapping = %#v, want %#v", actual, expected)
				}
				if bytes.Contains([]byte(raw), []byte("\r\n")) && bytes.Contains(bytes.ReplaceAll(updated, []byte("\r\n"), nil), []byte("\n")) {
					t.Fatalf("CRLF update introduced lone LF: %q", updated)
				}
			})
		}
	}
}
