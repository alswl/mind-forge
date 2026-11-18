package markdown

import (
	"strings"
	"testing"
)

func TestLegacyFrontMatterStripping(t *testing.T) {
	for _, test := range []struct {
		name, input, want string
		strip             func(string) string
	}{
		{"strip_typora_removes_only_that_key", "---\ntitle: Foo\ntypora-copy-images-to: ../assets\n---\nbody\n", "---\ntitle: Foo\n---\nbody\n", StripTyporaFrontMatter},
		{"strip_typora_noop_without_frontmatter", "no frontmatter here\n", "no frontmatter here\n", StripTyporaFrontMatter},
		{"strip_typora_noop_without_the_key", "---\ntitle: Foo\n---\nbody\n", "---\ntitle: Foo\n---\nbody\n", StripTyporaFrontMatter},
		{"strip_typora_drops_frontmatter_entirely_when_key_is_only_content", "---\ntypora-copy-images-to: ../assets\n---\nbody\n", "body\n", StripTyporaFrontMatter},
		{"strip_mind_forge_front_matter_removes_only_that_key", "---\ntitle: Foo\nmind-forge-visibility: private\n---\nbody\n", "---\ntitle: Foo\n---\nbody\n", StripMindForgeFrontMatter},
		{"strip_mind_forge_front_matter_drops_frontmatter_entirely_when_key_is_only_content", "---\nmind-forge-visibility: private\n---\nbody\n", "body\n", StripMindForgeFrontMatter},
		{"strip_mind_forge_front_matter_noop_without_the_key", "---\ntitle: Foo\n---\nbody\n", "---\ntitle: Foo\n---\nbody\n", StripMindForgeFrontMatter},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := test.strip(test.input); got != test.want {
				t.Fatalf("front matter bytes = %q, want %q", got, test.want)
			}
		})
	}
}

func TestLegacyBlockVisibility(t *testing.T) {
	for _, test := range []struct{ name, input, want string }{
		{"block_visibility_defaults_to_public_without_frontmatter", "# Title\nbody\n", "public"},
		{"block_visibility_defaults_to_public_without_the_key", "---\ntitle: Foo\n---\nbody\n", "public"},
		{"block_visibility_reads_explicit_public", "---\nmind-forge-visibility: public\n---\nbody\n", "public"},
		{"block_visibility_reads_private", "---\nmind-forge-visibility: private\n---\nbody\n", "private"},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, err := BlockVisibility(test.input)
			if err != nil || got != test.want {
				t.Fatalf("visibility = %q, %v; want %q", got, err, test.want)
			}
		})
	}
	t.Run("block_visibility_rejects_unrecognized_value", func(t *testing.T) {
		for _, bad := range []string{"internal", "privat", "true", "Private"} {
			_, err := BlockVisibility("---\nmind-forge-visibility: " + bad + "\n---\nbody\n")
			if err == nil || !strings.Contains(err.Error(), "'"+bad+"'") {
				t.Fatalf("invalid visibility %q was not rejected with its value: %v", bad, err)
			}
		}
	})
}
