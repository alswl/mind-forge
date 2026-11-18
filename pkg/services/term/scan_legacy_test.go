package term

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	textutil "github.com/alswl/mind-forge/pkg/common/text"
)

func TestLegacyWordPolicies(t *testing.T) {
	for _, test := range []struct{ name, original, match, boundary, want string }{
		{"word_check_for_substring_uses_requested_boundary", "anything", "substring", "loose", "loose"},
		{"word_check_for_substring_uses_requested_boundary", "anything", "substring", "standalone", "standalone"},
		{"word_check_for_ascii_word_picks_boundary", "aidc", "word", "loose", "ascii-loose"},
		{"word_check_for_ascii_word_picks_boundary", "aidc", "word", "standalone", "standalone"},
		{"word_check_for_ascii_phrase_with_space_is_ascii_boundary", "foo dr", "word", "loose", "ascii-loose"},
		{"word_check_for_ascii_phrase_with_space_is_ascii_boundary", "foo dr", "word", "standalone", "standalone"},
		{"word_check_for_cjk_original_is_cjk_regardless_of_boundary", "机器人", "word", "loose", "cjk"},
		{"word_check_for_cjk_original_is_cjk_regardless_of_boundary", "机器人", "word", "standalone", "cjk"},
		{"word_check_for_cjk_and_mixed_source_still_cjk", "机器", "word", "loose", "cjk"},
		{"word_check_for_cjk_and_mixed_source_still_cjk", "foo 机器", "word", "loose", "cjk"},
		{"word_check_for_cjk_and_mixed_source_still_cjk", "foo 机器", "word", "standalone", "cjk"},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := wordPolicy(model.Correction{Original: test.original, Match: test.match, Boundary: test.boundary})
			if got != test.want {
				t.Fatalf("word policy = %q, want %q", got, test.want)
			}
		})
	}
}

func TestLegacyIdentifierNeighbours(t *testing.T) {
	t.Run("is_identifier_neighbour_letters_digits_underscore", func(t *testing.T) {
		for _, b := range []byte("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_") {
			if !identifier(b) {
				t.Errorf("identifier byte %q rejected", b)
			}
		}
	})
	t.Run("is_identifier_neighbour_kebab_path_dot", func(t *testing.T) {
		for _, b := range []byte("-/\\.") {
			if !identifier(b) {
				t.Errorf("identifier byte %q rejected", b)
			}
		}
	})
	t.Run("is_identifier_neighbour_excludes_whitespace_and_punct", func(t *testing.T) {
		for _, b := range []byte(" \t\n\r,;:!?()[]{}\"'") {
			if identifier(b) {
				t.Errorf("punctuation %q accepted as identifier byte", b)
			}
		}
	})
}

func TestLegacyASCIIWordBoundaries(t *testing.T) {
	for _, test := range []struct {
		name, content, boundary string
		want                    bool
	}{
		{"loose_matches_inside_kebab_today", "xxx-aidc-test", "loose", true},
		{"loose_matches_standalone", "the aidc site", "loose", true},
		{"standalone_rejects_kebab_left_neighbour", "xxx-aidc test", "standalone", false},
		{"standalone_rejects_kebab_right_neighbour", "test aidc-suffix", "standalone", false},
		{"standalone_rejects_kebab_both_sides", "xxx-aidc-test", "standalone", false},
		{"standalone_rejects_snake_case", "my_aidc_db", "standalone", false},
		{"standalone_rejects_path_slash_neighbour", "./docs/aidc/intro.md", "standalone", false},
		{"standalone_rejects_dot_neighbour", "module.aidc.handler", "standalone", false},
		{"standalone_rejects_backslash_neighbour", `win\aidc\file`, "standalone", false},
		{"standalone_accepts_whitespace_neighbours", "the aidc site", "standalone", true},
		{"standalone_accepts_punctuation_neighbours", "(aidc) and aidc, then aidc.", "standalone", true},
		{"standalone_accepts_start_and_end_of_input", "aidc", "standalone", true},
		{"standalone_accepts_cjk_right_neighbour", "the aidc 站点", "standalone", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			start := strings.Index(test.content, "aidc")
			got := accepts(test.content, []byte(test.content), model.Correction{Original: "aidc", Match: "word", Boundary: test.boundary}, start, start+4, textutil.Segmentation{})
			if got != test.want {
				t.Fatalf("word boundary = %v, want %v", got, test.want)
			}
		})
	}
}

func TestLegacyReplacementPadding(t *testing.T) {
	for _, test := range []struct{ content, replacement, want string }{
		{"甲旧乙", "ASCII", " ASCII "},
		{"甲旧，", "装置", "装置"},
		{"甲旧乙", "DD 站点", " DD 站点"},
		{"甲旧 乙", "ASCII", " ASCII"},
	} {
		if got := padded(test.content, 3, 6, test.replacement); got != test.want {
			t.Errorf("padded replacement = %q, want %q", got, test.want)
		}
	}
}

func TestLegacyScanOffsetsAndMaskedMatching(t *testing.T) {
	for _, test := range []struct {
		name, content, original string
		count                   int
		line, column            uint32
	}{
		{"byte_offset_basic", "hello\nworld", "world", 1, 2, 1},
		{"byte_offset_first_line", "hello", "hello", 1, 1, 1},
		{"find_subseq_exact", "hello world", "world", 1, 1, 7},
		{"find_subseq_rejects_all_zeroes", "\x00\x00\x00\x00\x00\x00\x00\x00", "mindrepo", 0, 0, 0},
		{"find_subseq_rejects_zero_anywhere_in_window", "mind\x00epo", "mindrepo", 0, 0, 0},
		{"find_subseq_not_found", "hello world", "xyz", 0, 0, 0},
		{"cross_exempt_region_match_does_not_panic_or_flag", "库「a神」在这里。", "库神器", 0, 0, 0},
		{"exempt_region_without_ascii_stays_aligned_and_unmatched", "库「神」在这里。", "库神器", 0, 0, 0},
		{"short_original_before_exempt_region_stays_unmatched", "库「a神」在这里。", "库神", 0, 0, 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			r, err := Scan(test.content, "synthetic.md", []model.Term{termRule("Synthetic", test.original, "Synthetic", "word", "standalone")}, Selection{})
			if err != nil || len(r.Findings) != test.count {
				t.Fatalf("findings = %+v, %v; want count %d", r.Findings, err, test.count)
			}
			if test.count > 0 && (r.Findings[0].Line != test.line || r.Findings[0].Column != test.column) {
				t.Fatalf("position = %d:%d, want %d:%d", r.Findings[0].Line, r.Findings[0].Column, test.line, test.column)
			}
		})
	}
}

func TestLegacyCJKAndSubstringFindings(t *testing.T) {
	for _, test := range []struct {
		name, content, original, match string
		count                          int
		eligible                       bool
		reason                         string
	}{
		{"cjk_word_matches_complete_token_but_not_embedded_fragment", "机器人需要维护。机器 很常见。", "机器", "word", 1, false, "short-cjk-advisory"},
		{"word_mode_rejects_embedded_occurrence", "scatter cat", "cat", "word", 1, true, ""},
		{"short_cjk_spanning_word_boundary_is_not_flagged", "以可独立验证和回退的方案", "以可", "word", 0, false, ""},
		{"genuine_short_cjk_standalone_is_advisory", "机器 很常见", "机器", "word", 1, false, "short-cjk-advisory"},
		{"longer_cjk_correction_stays_replacement_eligible", "机器人 很常见", "机器人", "word", 1, true, ""},
		{"cjk_substring_is_not_advisory", "以可独立验证", "以可", "substring", 1, true, ""},
		{"substring_standalone_rejects_embedded_occurrence", "scatter cat", "cat", "substring", 1, true, ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			r, err := Scan(test.content, "synthetic.md", []model.Term{termRule("Synthetic", test.original, "Synthetic", test.match, "standalone")}, Selection{})
			if err != nil || len(r.Findings) != test.count {
				t.Fatalf("findings = %+v, %v; want count %d", r.Findings, err, test.count)
			}
			if test.count == 0 {
				return
			}
			f := r.Findings[0]
			reason := ""
			if f.SafetyReason != nil {
				reason = *f.SafetyReason
			}
			if f.Original != test.original || f.ReplacementEligible != test.eligible || reason != test.reason {
				t.Fatalf("finding classification = %+v; want original %q, eligible %v, reason %q", f, test.original, test.eligible, test.reason)
			}
		})
	}
}
