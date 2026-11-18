package markdown

import "testing"

func TestLegacyReferencePathSemantics(t *testing.T) {
	for _, test := range []struct {
		name, target, source, base, want string
		ok                               bool
	}{
		{"normalize_lexical_collapses_interior_dotdot", "docs/slug/../../assets/x.png", ".", ".", "assets/x.png", true},
		{"normalize_lexical_collapses_interior_dotdot", "docs/art/./assets/y.png", ".", ".", "docs/art/assets/y.png", true},
		{"normalize_lexical_collapses_interior_dotdot", "../shared/z.png", ".", ".", "../shared/z.png", true},
		{"relative_path_from_computes_up_and_down", "assets/x.png", ".", "outputs", "../assets/x.png", true},
		{"relative_path_from_computes_up_and_down", "a/b", ".", "a/b", ".", true},
		{"relative_path_from_computes_up_and_down", "a/b/c/d.png", ".", "a/b/out", "../c/d.png", true},
		{"relative_path_from_rejects_mixed_absolute_and_relative_bases", "x.png", "assets", "/abs/outputs", "", false},
		{"relative_path_from_rejects_mixed_absolute_and_relative_bases", "x.png", "/abs/assets", "outputs", "", false},
		{"relative_path_from_rejects_mixed_absolute_and_relative_bases", "x.png", "/repo/assets", "/repo/outputs", "../assets/x.png", true},
		{"relative_path_from_rejects_root_only_absolute_common_prefix", "x.png", "/other/repo/assets", "/tmp/out", "", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, ok := RebaseRelativeTarget(test.target, test.source, test.base)
			if ok != test.ok || got != test.want {
				t.Fatalf("rebased path = %q, %v; want %q, %v", got, ok, test.want, test.ok)
			}
		})
	}
}
