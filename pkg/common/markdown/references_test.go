package markdown

import (
	"strings"
	"testing"
)

func TestMergedReferencesKeepFencesAndExternalTargets(t *testing.T) {
	body := "![a](../../assets/a.png)\r\n[link](../notes.md)\n  [ref]: ../../assets/a.png \"title\"\n<img src='../../assets/a.png'>\n[remote](https://example.invalid/a.png)\n```md\n![literal](../../assets/a.png)\n```\n"
	got := RewriteReferences(body, func(target string) (string, bool) { return RebaseRelativeTarget(target, "docs/post", "docs") })
	for _, want := range []string{"![a](../assets/a.png)", "[link](notes.md)", "  [ref]: ../assets/a.png \"title\"", "<img src='../assets/a.png'>", "[remote](https://example.invalid/a.png)", "```md\n![literal](../../assets/a.png)\n```\n"} {
		if !strings.Contains(got, want) {
			t.Fatalf("missing %q in %q", want, got)
		}
	}
	if strings.Contains(got, "\r") {
		t.Fatal("reference rewrite did not normalize line endings")
	}
	if _, ok := RebaseRelativeTarget("asset.png", "/absolute/source", "relative"); ok {
		t.Fatal("mixed bases accepted")
	}
	if _, ok := RebaseRelativeTarget("asset.png", "/a/source", "/b/output"); ok {
		t.Fatal("unrelated absolute anchors accepted")
	}
}

func TestTyporaRemovalPreservesOtherFrontMatterAndCRLF(t *testing.T) {
	for _, c := range []struct{ input, want string }{
		{"---\ntypora-copy-images-to: ../assets\n---\n\n# Title\n", "# Title\n"},
		{"---\r\ntitle: Keep\r\ntypora-copy-images-to: ../assets\r\n---\r\n# Title\r\n", "---\r\ntitle: Keep\r\n---\r\n# Title\r\n"},
		{"---\ninvalid", "---\ninvalid"},
	} {
		if got := StripTyporaFrontMatter(c.input); got != c.want {
			t.Errorf("%q: %q want %q", c.input, got, c.want)
		}
	}
}
