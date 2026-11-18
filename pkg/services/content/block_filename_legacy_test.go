package content

import "testing"

func TestLegacyBlockNumberPrefix(t *testing.T) {
	for _, group := range []struct {
		name  string
		cases [][2]string
	}{
		{"extract_prefix_basic", [][2]string{{"02-notes.md", "02"}, {"01-opening.md", "01"}, {"10-body.md", "10"}}},
		{"extract_prefix_no_extension", [][2]string{{"02-notes", "02"}}},
		{"extract_prefix_no_prefix", [][2]string{{"notes.md", ""}, {"introduction.md", ""}}},
	} {
		t.Run(group.name, func(t *testing.T) {
			for _, test := range group.cases {
				if got := blockNumberPrefix(test[0]); got != test[1] {
					t.Errorf("prefix(%q) = %q, want %q", test[0], got, test[1])
				}
			}
		})
	}
}

func TestLegacyBlockFilenameSlug(t *testing.T) {
	for _, group := range []struct {
		name  string
		cases [][2]string
	}{
		{"extract_slug_basic", [][2]string{{"02-notes.md", "notes"}, {"01-opening.md", "opening"}, {"05-alternatives-considered.md", "alternatives-considered"}}},
		{"extract_slug_no_prefix", [][2]string{{"notes.md", "notes"}}},
	} {
		t.Run(group.name, func(t *testing.T) {
			for _, test := range group.cases {
				if got := blockFilenameSlug(test[0]); got != test[1] {
					t.Errorf("slug(%q) = %q, want %q", test[0], got, test[1])
				}
			}
		})
	}
}

func TestBlockFilenameParsingKeepsSelectorAndRewriteRulesDistinct(t *testing.T) {
	for _, test := range []struct{ name, prefix, selector, rewrite string }{
		{"notes.md", "", "notes", "notes.md"},
		{"notes-extra.md", "", "notes-extra", "extra"},
		{"123.md", "123", "", "123.md"},
		{"02---notes.md", "02", "notes", "--notes"},
	} {
		if prefix, slug, rewrite := blockNumberPrefix(test.name), blockFilenameSlug(test.name), blockSlug(test.name); prefix != test.prefix || slug != test.selector || rewrite != test.rewrite {
			t.Errorf("filename %q: prefix=%q selector slug=%q rewrite slug=%q", test.name, prefix, slug, rewrite)
		}
	}
}

func TestBlockSelectorRemovesAllLeadingSeparatorHyphens(t *testing.T) {
	got, err := ResolveBlockFilename([]string{"docs/a/02---notes.md"}, "docs/a", "notes")
	if err != nil || got != "02---notes.md" {
		t.Fatalf("selector = %q, %v", got, err)
	}
}
