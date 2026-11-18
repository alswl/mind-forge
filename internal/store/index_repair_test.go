package store

import (
	"bytes"
	"testing"
)

func TestDuplicateIndexRepairMergesOnlyAffectedSections(t *testing.T) {
	raw := []byte("schema: '1'\nterms:\n- a\nterms:\n- b\ncustom: {note: retained} # exact\nassets:\n  first: {name: first}\nassets:\n  second: {name: second}\n")
	duplicates, err := IndexDuplicates(raw)
	if err != nil || len(duplicates) != 2 || duplicates[0].Key != "terms" {
		t.Fatalf("%#v %v", duplicates, err)
	}
	fixed, err := MergeDuplicateIndex(raw)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(fixed, []byte("custom: {note: retained} # exact\n")) {
		t.Fatal("repair changed unrelated section")
	}
	duplicates, err = IndexDuplicates(fixed)
	if err != nil || len(duplicates) != 0 {
		t.Fatalf("%s %#v %v", fixed, duplicates, err)
	}
	doc, err := ParseYAML(fixed)
	if err != nil {
		t.Fatal(err)
	}
	var value map[string]any
	if err := doc.Decode(&value); err != nil {
		t.Fatal(err)
	}
	if len(value["terms"].([]any)) != 2 || len(value["assets"].(map[string]any)) != 2 {
		t.Fatalf("merge lost rows: %s", fixed)
	}
}
