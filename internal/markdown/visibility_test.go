package markdown

import "testing"

func TestVisibilityRequiresCompleteLeadingFrontMatter(t *testing.T) {
	cases := []struct {
		body, want string
		fails      bool
	}{
		{"# Title\n", "public", false},
		{"---\nmind-forge-visibility: private\n---\n# Title\n", "private", false},
		{"---\r\nmind-forge-visibility: 'public'\r\n---\r\n", "public", false},
		{"---\n\u2003mind-forge-visibility: private\n---\n", "private", false},
		{"---\nmind-forge-visibility: private\n", "public", false},
		{"---\nmind-forge-visibility: PRIVATE\n---\n", "", true},
	}
	for _, c := range cases {
		got, err := BlockVisibility(c.body)
		if got != c.want || (err != nil) != c.fails {
			t.Errorf("%q: %s %v", c.body, got, err)
		}
	}
}
