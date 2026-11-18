package markdown

import (
	"reflect"
	"strings"
	"testing"
)

func TestLegacyFenceTracker(t *testing.T) {
	for _, test := range []struct {
		name, input string
		want        []bool
	}{
		{"simple_backtick_fence", "before\n```\ninside\n```\nafter", []bool{false, true, true, true, false}},
		{"simple_tilde_fence", "before\n~~~\ninside\n~~~\nafter", []bool{false, true, true, true, false}},
		{"longer_backtick_fence", "````\nfenced\n````\noutside", []bool{true, true, true, false}},
		{"longer_open_short_close_does_not_close", "````\na\n```\nb", []bool{true, true, true, true}},
		{"longer_close_than_open_is_fine", "```\na\n`````\nb", []bool{true, true, true, false}},
		{"backtick_inside_tilde_stays_open", "~~~\nbody\n```\nmore\n~~~", []bool{true, true, true, true, true}},
		{"tilde_inside_backtick_stays_open", "```\nbody\n~~~\nmore\n```", []bool{true, true, true, true, true}},
		{"indented_4_spaces_is_not_fence", "    ```\nnot fenced\n    ```", []bool{false, false, false}},
		{"indented_3_spaces_is_fence", "   ```\ninside\n   ```\noutside", []bool{true, true, true, false}},
		{"unterminated_fence_stays_open", "```\na\nb", []bool{true, true, true}},
		{"backtick_fence_with_info_string", "```rust\nfn main() {}\n```", []bool{true, true, true}},
		{"backtick_fence_with_leading_spaces_and_info", "  ``` rust,ignore\ncode\n```", []bool{true, true, true}},
		{"tilde_with_extra_text_is_not_fence", "~~~rust\nthis is not a fence\n~~~", []bool{false, false, true}},
		{"backtick_in_info_string_is_not_fence", "``` `\ncode\n```", []bool{false, false, true}},
		{"fence_chars_mid_line_are_not_fence", "text ```\nnot a fence\n```", []bool{false, false, true}},
	} {
		t.Run(test.name, func(t *testing.T) {
			var tracker FenceTracker
			got := []bool{}
			for _, line := range strings.Split(test.input, "\n") {
				got = append(got, tracker.ProcessLine(line))
			}
			if !reflect.DeepEqual(got, test.want) {
				t.Fatalf("fenced lines = %v, want %v", got, test.want)
			}
		})
	}
}
