package content

import "testing"

func TestArticleFilenameUsesFrozenLowercaseAndUnicodeAlphabetic(t *testing.T) {
	for _, tc := range []struct{ input, want string }{
		{"Hello World", "hello-world"}, {"Foo_Bar.Baz", "foo-bar-baz"}, {"  --a--  ", "a"},
		{"!!!", "untitled"}, {"Agent 本周进展", "agent-本周进展"}, {"ΟΣ", "ος"},
		{"\U000105C0", "\U000105C0"}, {"a\u0345b", "a\u0345b"}, {"a\u0301b", "a-b"},
	} {
		if got := ArticleFilename(tc.input); got != tc.want {
			t.Errorf("%q: got %q want %q", tc.input, got, tc.want)
		}
	}
}

func TestTyporaFrontMatterPreservesExistingBytes(t *testing.T) {
	for _, tc := range []struct{ input, want string }{
		{"# Title\n", "---\ntypora-copy-images-to: ../assets\n---\n\n# Title\n"},
		{"---\ntitle: T\n---\n# Title\n", "---\ntitle: T\ntypora-copy-images-to: ../assets\n---\n# Title\n"},
		{"---\r\ntitle: T\r\n---\r\n# Title\r\n", "---\r\ntitle: T\r\ntypora-copy-images-to: ../assets\r\n---\r\n# Title\r\n"},
		{"---\ntypora-copy-images-to: custom\n---\nBody", "---\ntypora-copy-images-to: custom\n---\nBody"},
		{"---\ntitle: T\n---", "---\ntitle: T\ntypora-copy-images-to: ../assets\n---\n"},
		{"---\nmalformed", "---\ntypora-copy-images-to: ../assets\n---\n\n---\nmalformed"},
	} {
		if got := InjectTyporaFrontMatter(tc.input, "../assets"); got != tc.want {
			t.Errorf("%q: got %q want %q", tc.input, got, tc.want)
		}
	}
}
