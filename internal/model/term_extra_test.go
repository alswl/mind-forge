package model

import (
	"encoding/json"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestTermOpaqueYAMLFieldsSurviveTypedEditWithoutJSONExpansion(t *testing.T) {
	input := "term: API\ndefinition: meaning\ncustom_payload:\n  marker: opaque-marker\n  values: [true, 42, null]\n"
	var tm Term
	if err := yaml.Unmarshal([]byte(input), &tm); err != nil {
		t.Fatal(err)
	}
	description := "edited"
	tm.Description = &description
	raw, err := yaml.Marshal(tm)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), "opaque-marker") || !strings.Contains(string(raw), "custom_payload:") {
		t.Fatalf("opaque fields lost: %s", raw)
	}
	var roundtrip Term
	if err := yaml.Unmarshal(raw, &roundtrip); err != nil {
		t.Fatal(err)
	}
	if len(roundtrip.Extra) != 1 {
		t.Fatalf("unknown fields: %+v", roundtrip.Extra)
	}
	raw, err = json.Marshal(tm)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(raw), "custom_payload") || strings.Contains(string(raw), "opaque-marker") {
		t.Fatalf("typed JSON response expanded: %s", raw)
	}
}
