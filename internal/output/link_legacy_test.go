package output

import (
	"strings"
	"testing"
)

func TestLegacyHyperlinks(t *testing.T) {
	t.Run("hyperlink_wraps_with_osc8", func(t *testing.T) {
		got := RenderLink("docs/getting-started.md", "file://docs/getting-started.md", true)
		if !strings.HasPrefix(got, "\x1b]8;;") || !strings.Contains(got, "docs/getting-started.md") || !strings.HasSuffix(got, "\x1b]8;;\x1b\\") {
			t.Fatalf("OSC8 envelope changed: %q", got)
		}
	})
	t.Run("plain_fallback_returns_visible_only", func(t *testing.T) {
		got := RenderLink("docs/getting-started.md", "file://docs/getting-started.md", false)
		if got != "docs/getting-started.md" || strings.ContainsRune(got, '\x1b') {
			t.Fatalf("plain link changed: %q", got)
		}
	})
	t.Run("visible_text_is_always_present", func(t *testing.T) {
		for _, enabled := range []bool{true, false} {
			if got := RenderLink("click here: https://example.com", "https://example.com", enabled); !strings.Contains(got, "click here: https://example.com") {
				t.Fatalf("visible target lost: %q", got)
			}
		}
	})
	for _, test := range []struct {
		name, path, base, uri string
		enabled               bool
		plain                 bool
	}{
		{"path_link_uses_file_uri", "docs/readme.md", "/home/user", "file:///home/user/docs/readme.md", true, false},
		{"path_link_plain_fallback", "docs/readme.md", "/home/user", "", false, true},
		{"path_link_absolute_path_passthrough", "/etc/hosts", "/repo", "file:///etc/hosts", true, false},
		{"path_link_no_base_returns_plain", "projects/demo", "", "", true, true},
		{"path_link_encodes_spaces_in_uri", "my docs/read me.md", "/home/user", "file:///home/user/my%20docs/read%20me.md", true, false},
		{"encode_file_path_spaces", "/tmp/my file.md", "/repo", "file:///tmp/my%20file.md", true, false},
		{"encode_file_path_unicode", "/tmp/设计.md", "/repo", "file:///tmp/%E8%AE%BE%E8%AE%A1.md", true, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := RenderPathLink(test.path, DisplayOptions{RepoRoot: test.base, Hyperlinks: test.enabled})
			if test.plain {
				if got != test.path || strings.ContainsRune(got, '\x1b') {
					t.Fatalf("plain path changed: %q", got)
				}
				return
			}
			if !strings.Contains(got, test.uri) || !strings.Contains(got, test.path) {
				t.Fatalf("path URI/label changed: %q", got)
			}
		})
	}
}
