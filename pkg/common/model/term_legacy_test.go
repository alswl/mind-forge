package model

import (
	"encoding/json"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestLegacyCorrectionModels(t *testing.T) {
	for _, test := range []struct{ name, input string }{
		{"correction_defaults_not_serialized", "original: mindrepo\ncorrect: Mind Repo\nmatch: word\nfix: required\nboundary: standalone\n"},
		{"correction_explicit_values_roundtrip", "original: 时刻表\ncorrect: 时刻表\nmatch: pinyin\nfix: suggested\nboundary: loose\npinyin: shi-ke-biao\n"},
		{"old_yaml_without_new_fields_loads_defaults", "original: mindrepo\ncorrect: Mind Repo\n"},
		{"boundary_default_is_standalone", "original: aidc\ncorrect: AIDC\n"},
		{"boundary_standalone_is_default_omitted_on_write", "original: aidc\ncorrect: AIDC\nboundary: standalone\n"},
		{"boundary_loose_is_explicit_and_serialized", "original: aidc\ncorrect: AIDC\nboundary: loose\n"},
		{"boundary_invalid_variant_fails", "original: foo\ncorrect: bar\nboundary: bogus\n"},
		{"match_kind_default_is_word", "original: mindrepo\ncorrect: Mind Repo\n"},
		{"fix_kind_default_is_required", "original: mindrepo\ncorrect: Mind Repo\n"},
		{"match_kind_invalid_variant_fails", "original: foo\ncorrect: bar\nmatch: bogus\n"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var c Correction
			err := yaml.Unmarshal([]byte(test.input), &c)
			if strings.Contains(test.name, "invalid_variant") {
				if err == nil {
					t.Fatal("invalid enum accepted")
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			raw, err := yaml.Marshal(c)
			if err != nil {
				t.Fatal(err)
			}
			switch test.name {
			case "correction_defaults_not_serialized":
				for _, key := range []string{"match:", "fix:", "boundary:", "pinyin:"} {
					if strings.Contains(string(raw), key) {
						t.Fatalf("default field serialized: %s", raw)
					}
				}
			case "correction_explicit_values_roundtrip":
				var again Correction
				if err := yaml.Unmarshal(raw, &again); err != nil {
					t.Fatal(err)
				}
				legacyConfigEqual(t, again.Match, "pinyin")
				legacyConfigEqual(t, again.Fix, "suggested")
				legacyConfigPointer(t, again.Pinyin, "shi-ke-biao")
			case "old_yaml_without_new_fields_loads_defaults":
				legacyConfigEqual(t, c.Original, "mindrepo")
				legacyConfigEqual(t, c.Correct, "Mind Repo")
				legacyConfigEqual(t, c.Match, "word")
				legacyConfigEqual(t, c.Fix, "required")
				legacyConfigEqual(t, c.Boundary, "standalone")
				if c.Pinyin != nil {
					t.Fatal("absent pinyin populated")
				}
			case "boundary_default_is_standalone":
				legacyConfigEqual(t, c.Boundary, "standalone")
				legacyConfigEqual(t, c.compact().Boundary, "")
				c.Boundary = "loose"
				legacyConfigEqual(t, c.compact().Boundary, "loose")
			case "boundary_standalone_is_default_omitted_on_write":
				legacyConfigEqual(t, c.Boundary, "standalone")
				if strings.Contains(string(raw), "boundary:") {
					t.Fatalf("default boundary serialized: %s", raw)
				}
			case "boundary_loose_is_explicit_and_serialized":
				if !strings.Contains(string(raw), "boundary: loose") {
					t.Fatalf("loose boundary lost: %s", raw)
				}
			case "match_kind_default_is_word":
				legacyConfigEqual(t, c.Match, "word")
				legacyConfigEqual(t, c.compact().Match, "")
				for _, kind := range []string{"substring", "pinyin"} {
					c.Match = kind
					legacyConfigEqual(t, c.compact().Match, kind)
				}
			case "fix_kind_default_is_required":
				legacyConfigEqual(t, c.Fix, "required")
				legacyConfigEqual(t, c.compact().Fix, "")
				c.Fix = "suggested"
				legacyConfigEqual(t, c.compact().Fix, "suggested")
			}
		})
	}
}

func TestLegacyCorrectionValidation(t *testing.T) {
	c := func(original, match, boundary string) Correction {
		return Correction{Original: original, Correct: "X", Match: match, Fix: "required", Boundary: boundary}
	}
	for _, test := range []struct {
		name        string
		corrections []Correction
		bad         bool
	}{
		{"validate_corrections_accepts_supported_matches_with_loose_boundary", []Correction{c("a", "word", "loose"), c("c", "pinyin", "loose")}, false},
		{"validate_corrections_accepts_substring_with_both_boundaries", []Correction{c("aidc", "substring", "loose"), c("sample", "substring", "standalone")}, false},
		{"validate_corrections_allows_pinyin_with_standalone", []Correction{c("shikebiao", "pinyin", "standalone")}, false},
		{"validate_corrections_rejects_edge_hyphen", []Correction{c("aidc-", "word", "standalone")}, true},
		{"validate_corrections_rejects_edge_underscore_leading", []Correction{c("_aidc", "word", "standalone")}, true},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := ValidateCorrections([]Term{{Term: "TEST", Corrections: test.corrections}})
			if (err != nil) != test.bad {
				t.Fatalf("correction validation: %v; reject=%v", err, test.bad)
			}
			if test.bad && !strings.Contains(err.Error(), "identifier-character edges") {
				t.Fatalf("missing edge diagnostic: %v", err)
			}
		})
	}
}

func TestLegacyTermReportModels(t *testing.T) {
	text, kind := "fix", "substring"
	var cleared *string
	for _, test := range []struct {
		name   string
		value  any
		want   map[string]any
		absent []string
		empty  bool
	}{
		{"correction_selector_serialization", CorrectionSelector{Original: "typo"}, map[string]any{"original": "typo"}, nil, false},
		{"correction_patch_is_empty_when_all_none", CorrectionPatch{}, nil, nil, true},
		{"correction_patch_is_not_empty_with_correct", CorrectionPatch{Correct: &text}, map[string]any{"correct": "fix"}, nil, false},
		{"correction_patch_is_not_empty_with_match_kind", CorrectionPatch{Match: &kind}, map[string]any{"match": "substring"}, nil, false},
		{"correction_patch_serialization_skips_none", CorrectionPatch{Match: stringPointerForLegacy("pinyin")}, map[string]any{"match": "pinyin"}, []string{"correct"}, false},
		{"correction_patch_pinyin_clear", CorrectionPatch{Pinyin: &cleared}, map[string]any{"pinyin": nil}, nil, false},
		{"correction_change_report_serialization", CorrectionChangeReport{Term: "RAG", Scope: "project", Correction: Correction{Original: "rag", Correct: "RAG", Match: "word", Fix: "required", Boundary: "standalone"}, Changes: []string{"correct: rag → RAG"}}, map[string]any{"term": "RAG", "scope": "project", "dry_run": false, "changes": []any{"correct: rag → RAG"}}, nil, false},
		{"term_move_report_serialization", TermMoveReport{Term: "RAG", FromScope: "project", ToScope: "global", DryRun: true, Force: false, SideEffects: []MoveSideEffect{}}, map[string]any{"term": "RAG", "from_scope": "project", "to_scope": "global", "dry_run": true, "force": false}, nil, false},
		{"move_side_effect_serialization", MoveSideEffect{Action: "remove", Scope: "project", Description: "remove RAG from project alpha"}, map[string]any{"action": "remove", "scope": "project"}, nil, false},
		{"term_check_target_serialization", TermCheckTarget{TargetType: "article", Project: stringPointerForLegacy("alpha"), Article: stringPointerForLegacy("weekly-note")}, map[string]any{"target_type": "article", "project": "alpha", "article": "weekly-note"}, []string{"path"}, false},
		{"term_check_target_file_type", TermCheckTarget{TargetType: "file", Path: stringPointerForLegacy("docs/draft.md")}, map[string]any{"target_type": "file", "path": "docs/draft.md"}, nil, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			raw, err := json.Marshal(test.value)
			if err != nil {
				t.Fatal(err)
			}
			var actual map[string]any
			if err := json.Unmarshal(raw, &actual); err != nil {
				t.Fatal(err)
			}
			if test.empty && len(actual) != 0 {
				t.Fatalf("empty patch serialized fields: %s", raw)
			}
			for key, want := range test.want {
				got, exists := actual[key]
				if !exists {
					t.Fatalf("field %q absent: %s", key, raw)
				}
				legacyConfigEqual(t, got, want)
			}
			for _, key := range test.absent {
				if _, exists := actual[key]; exists {
					t.Fatalf("optional field %q present: %s", key, raw)
				}
			}
		})
	}
}

func stringPointerForLegacy(value string) *string { return &value }
