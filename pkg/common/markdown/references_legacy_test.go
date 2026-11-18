package markdown

import (
	"strings"
	"testing"
)

func TestLegacyReferenceRewriting(t *testing.T) {
	for _, test := range []struct{ name, input, want string }{
		{"rewrite_line_preserves_alt_text_and_rewrites_only_path", "![工作流全景](assets/p.png)", "![工作流全景](../docs/art/assets/p.png)"},
		{"rewrite_line_handles_multiple_links_on_one_line", "see ![a](x.png) and [b](y.md) too", "see ![a](../docs/art/x.png) and [b](../docs/art/y.md) too"},
		{"rewrite_line_rewrites_reference_definition", "[img]: assets/p.png", "[img]: ../docs/art/assets/p.png"},
		{"rewrite_line_rewrites_reference_definition", "  [img]: assets/p.png \"Title\"", "  [img]: ../docs/art/assets/p.png \"Title\""},
		{"rewrite_line_leaves_non_links_untouched", "plain text, no links here", "plain text, no links here"},
		{"rewrite_line_leaves_non_links_untouched", "stray ](x.png) bracket", "stray ](x.png) bracket"},
		{"rewrite_line_leaves_non_links_untouched", "[a](https://x/y) ![b](/abs.png)", "[a](https://x/y) ![b](/abs.png)"},
		{"rewrite_html_img_src_double_quotes", `<img src="assets/p.png" alt="pic">`, `<img src="../docs/art/assets/p.png" alt="pic">`},
		{"rewrite_html_img_src_single_quotes", "<img src='assets/p.png'>", "<img src='../docs/art/assets/p.png'>"},
		{"rewrite_html_img_src_preserves_absolute_and_urls", `<img src="/abs/logo.png">`, `<img src="/abs/logo.png">`},
		{"rewrite_html_img_src_preserves_absolute_and_urls", `<img src="https://x/y.png">`, `<img src="https://x/y.png">`},
		{"rewrite_html_img_src_alongside_markdown_image_on_same_line", `![a](x.png) and <img src="y.png">`, `![a](../docs/art/x.png) and <img src="../docs/art/y.png">`},
		{"rewrite_html_img_src_multiple_tags_on_one_line", `<img src="a.png"> text <img src="b.png">`, `<img src="../docs/art/a.png"> text <img src="../docs/art/b.png">`},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := rewriteLineReferences(test.input, func(target string) (string, bool) {
				return RebaseRelativeTarget(target, "docs/art", "outputs")
			})
			if got != test.want {
				t.Fatalf("rewritten bytes = %q, want %q", got, test.want)
			}
			if test.name == "rewrite_line_preserves_alt_text_and_rewrites_only_path" && (strings.Contains(got, "![工作流全景![") || strings.Count(got, "](") != 1) {
				t.Fatalf("alt text or link count changed: %q", got)
			}
		})
	}
	t.Run("rewrite_html_img_src_no_tag_returns_original", func(t *testing.T) {
		if got := rewriteLineReferences("plain text", func(string) (string, bool) { return "unused", true }); got != "plain text" {
			t.Fatalf("unrelated bytes changed: %q", got)
		}
	})
	t.Run("rewrite_references_skips_fenced_code_blocks", func(t *testing.T) {
		got := RewriteReferences("before ![a](x.png)\n```\n![b](y.png)\n```\nafter ![c](z.png)\n", func(target string) (string, bool) {
			return RebaseRelativeTarget(target, "docs/art", "outputs")
		})
		want := "before ![a](../docs/art/x.png)\n```\n![b](y.png)\n```\nafter ![c](../docs/art/z.png)\n"
		if got != want {
			t.Fatalf("fenced references = %q, want %q", got, want)
		}
	})
}

func TestLegacyRewritableTargets(t *testing.T) {
	for _, target := range []string{"/abs/x.png", "http://x/y.png", "https://x/y.png", "mailto:a@b.com", "data:image/png,AA", "#anchor", ""} {
		if ShouldRewriteTarget(target) {
			t.Errorf("must not rewrite %q", target)
		}
	}
	if !ShouldRewriteTarget("assets/p.png") {
		t.Fatal("relative asset target was rejected")
	}
}
