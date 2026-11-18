package build

import "testing"

func TestLegacyPrivateCallouts(t *testing.T) {
	for _, test := range []struct {
		name, input, want string
		warnings          int
		checkLine         bool
	}{
		{"strip_private_callouts_removes_short_form", "Before.\n\n> [!mf-private]\n> Secret note.\n\nAfter.\n", "Before.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_removes_explicit_alias", "Before.\n\n> [!mind-forge-private]\n> Secret note.\n\nAfter.\n", "Before.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_is_case_insensitive", "Before.\n\n> [!MF-Private]\n> Secret.\n\nAfter.\n", "Before.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_removes_multi_paragraph_list_and_nested", "Before.\n\n> [!mf-private] title\n> Paragraph one.\n>\n> - item a\n> - item b\n>\n> > [!mf-private] nested\n> > still private\n\nAfter.\n", "Before.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_merges_across_bare_blank_line_and_warns", "Before.\n\n> [!mf-private]\n> front half\n\n> back half\n\nAfter.\n", "Before.\n\nAfter.\n", 1, true},
		{"strip_private_callouts_merges_across_multiple_consecutive_bare_blank_lines", "Before.\n\n> [!mf-private]\n> front half\n\n\n> back half\n\nAfter.\n", "Before.\n\nAfter.\n", 1, false},
		{"strip_private_callouts_bare_blank_line_with_no_further_quote_ends_normally", "Before.\n\n> [!mf-private]\n> Secret.\n\nAfter blank, not part of the callout.\n", "Before.\n\nAfter blank, not part of the callout.\n", 0, false},
		{"strip_private_callouts_bare_blank_line_before_fenced_block_ends_normally", "Before.\n\n> [!mf-private]\n> Secret.\n\n```\n> looks like a quote but is code\n```\nAfter.\n", "Before.\n\n```\n> looks like a quote but is code\n```\nAfter.\n", 0, false},
		{"strip_private_callouts_bare_blank_line_at_end_of_file_ends_normally", "Before.\n\n> [!mf-private]\n> Secret.\n\n", "Before.\n\n", 0, false},
		{"strip_private_callouts_merges_trailing_public_blockquote_across_blank_line", "Before.\n\n> [!mf-private]\n> Secret.\n\n> Not private, but still swallowed.\n\nAfter.\n", "Before.\n\nAfter.\n", 1, false},
		{"strip_private_callouts_leaves_fenced_example_untouched", "Before.\n```\n> [!mf-private]\n> shown as an example\n```\nAfter.\n", "Before.\n```\n> [!mf-private]\n> shown as an example\n```\nAfter.\n", 0, false},
		{"strip_private_callouts_leaves_non_private_callout_untouched", "Before.\n\n> [!note]\n> This is a normal callout.\n\nAfter.\n", "Before.\n\n> [!note]\n> This is a normal callout.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_is_idempotent", "Before.\n\n> [!mf-private]\n> Secret.\n\nAfter.\n", "Before.\n\nAfter.\n", 0, false},
		{"strip_private_callouts_noop_without_markers", "# Title\n\nJust ordinary prose with no callouts at all.\n", "# Title\n\nJust ordinary prose with no callouts at all.\n", 0, false},
		{"strip_private_callouts_at_start_of_section_leaves_single_blank_line", "## Head\n> [!mf-private]\n> secret\n\nAfter.\n", "## Head\n\nAfter.\n", 0, false},
		{"strip_private_callouts_immediately_followed_by_next_heading", "Before.\n\n> [!mf-private]\n> secret\n## Next\n", "Before.\n\n## Next\n", 0, false},
		{"strip_private_callouts_as_only_content_of_a_section", "## A\n\n> [!mf-private]\n> secret\n\n## B\n", "## A\n\n## B\n", 0, false},
		{"strip_private_callouts_preserves_legitimate_multi_blank_formatting", "Before.\n\n\n\nAfter still here.\n", "Before.\n\n\n\nAfter still here.\n", 0, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, warnings := StripPrivateCallouts(test.input)
			if got != test.want {
				t.Fatalf("stripped bytes = %q, want %q", got, test.want)
			}
			if len(warnings) != test.warnings {
				t.Fatalf("warnings = %+v, want count %d", warnings, test.warnings)
			}
			if test.checkLine && warnings[0].StartLine != 3 {
				t.Fatalf("warning start line = %d, want 3", warnings[0].StartLine)
			}
			if test.name == "strip_private_callouts_is_idempotent" {
				again, _ := StripPrivateCallouts(got)
				if again != got {
					t.Fatal("stripping is not idempotent")
				}
			}
		})
	}
}
