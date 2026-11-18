package model

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestViewCollectionsAndOptionalFieldsHaveDistinctWireShapes(t *testing.T) {
	for _, item := range []struct {
		value            any
		contains, absent []string
	}{
		{ProjectRemoveReport{}, []string{`"references":[]`, `"side_effects":[]`}, []string{`"after"`}},
		{AssetRemoveReport{}, nil, []string{`"references"`, `"side_effects"`}},
		{AssetIndexReport{}, []string{`"added":[]`, `"removed":[]`}, []string{`"refreshed"`}},
		{TermLintReport{}, []string{`"findings":[]`, `"skipped_files":[]`, `"failures":[]`}, []string{`"would_fix_count"`, `"quote_protected_count"`}},
		{ProjectDetails{}, []string{`"last_active":null`, `"mind_yaml_summary":null`, `"layout":null`}, nil},
	} {
		raw, err := json.Marshal(item.value)
		if err != nil {
			t.Fatal(err)
		}
		for _, value := range item.contains {
			if !strings.Contains(string(raw), value) {
				t.Errorf("%T missing %s: %s", item.value, value, raw)
			}
		}
		for _, value := range item.absent {
			if strings.Contains(string(raw), value) {
				t.Errorf("%T unexpectedly contains %s: %s", item.value, value, raw)
			}
		}
	}
	zero := uint64(0)
	empty := JSONArray[AssetUpdateResult]{}
	raw, err := json.Marshal(AssetIndexReport{Refreshed: &empty})
	if err != nil || !strings.Contains(string(raw), `"refreshed":[]`) {
		t.Fatalf("explicit empty optional collection: %s (%v)", raw, err)
	}
	raw, err = json.Marshal(TermLintReport{WouldFixCount: &zero})
	if err != nil || !strings.Contains(string(raw), `"would_fix_count":0`) {
		t.Fatalf("explicit optional zero: %s (%v)", raw, err)
	}
}

func TestCorrectionPatchPreservesUnchangedClearAndSet(t *testing.T) {
	var clear *string
	value := "reading"
	set := &value
	for _, item := range []struct {
		patch CorrectionPatch
		want  string
	}{{CorrectionPatch{}, `{}`}, {CorrectionPatch{Pinyin: &clear}, `{"pinyin":null}`}, {CorrectionPatch{Pinyin: &set}, `{"pinyin":"reading"}`}} {
		raw, err := json.Marshal(item.patch)
		if err != nil || string(raw) != item.want {
			t.Fatalf("patch: %s (%v), want %s", raw, err, item.want)
		}
	}
}

func TestPublishVariantTagAndSourceRemovalFlattening(t *testing.T) {
	raw, err := json.Marshal(PublishRunOutcome{Local: &LocalRunOutcome{TargetName: "local"}})
	if err != nil || !strings.Contains(string(raw), `"target_type":"local"`) || strings.Contains(string(raw), `"effective_date"`) {
		t.Fatalf("local variant: %s (%v)", raw, err)
	}
	raw, err = json.Marshal(PublishRunOutcome{YuquePrompt: &YuquePromptRunOutcome{}})
	if err != nil || !strings.Contains(string(raw), `"target_type":"yuque-prompt"`) || !strings.Contains(string(raw), `"svg_png_replaced":[]`) || strings.Contains(string(raw), `"destination"`) {
		t.Fatalf("yuque variant: %s (%v)", raw, err)
	}
	raw, err = json.Marshal(SourceRemoveReport{Source: Source{Name: "example", Kind: "file"}, FileDeleted: true})
	if err != nil || !strings.Contains(string(raw), `"name":"example"`) || !strings.Contains(string(raw), `"file_deleted":true`) || strings.Contains(string(raw), `"source"`) {
		t.Fatalf("flattened source removal: %s (%v)", raw, err)
	}
}

func TestLifecycleReportsUseOriginalDeterministicOrdering(t *testing.T) {
	empty, path := "", "docs/a.md"
	references := []Reference{{FromPath: &path, FromID: "z"}, {FromID: "z"}, {FromPath: &empty, FromID: "a"}, {FromPath: &path, FromID: "a"}}
	SortReferences(references)
	if references[0].FromPath != nil || references[1].FromPath != &empty || references[2].FromID != "a" || references[3].FromID != "z" {
		t.Fatal(references)
	}
	changes := []PlannedChange{{Path: "b", Op: "remove_file"}, {Path: "a", Op: "refresh_index"}, {Path: "a", Op: "rename_file"}}
	SortPlannedChanges(changes)
	if changes[0].Op != "rename_file" || changes[1].Op != "refresh_index" || changes[2].Path != "b" {
		t.Fatal(changes)
	}
}
