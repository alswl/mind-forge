package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProjectListRetainsIndexDiagnosticsAndRows(t *testing.T) {
	for _, test := range []struct{ name, raw, want string }{
		{"malformed", "schema: '1'\nnot: : valid: yaml\n", "line 2"},
		{"duplicate", "schema: '1'\narticles: []\narticles: []\n", "duplicate key"},
		{"valid", "schema: '1'\n", ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [lab]\n")
			putContent(t, root, "projects/lab/mind.yaml", "schema: '1'\n")
			path := filepath.Join(root, "projects/lab/mind-index.yaml")
			putContent(t, root, "projects/lab/mind-index.yaml", test.raw)
			rows, warnings, err := ListProjects(root)
			if err != nil || len(rows) != 1 || rows[0].Name != "lab" || rows[0].DocumentCount != 0 {
				t.Fatalf("rows=%+v warnings=%v err=%v", rows, warnings, err)
			}
			if test.want == "" {
				if len(warnings) != 0 {
					t.Fatalf("unexpected warnings: %v", warnings)
				}
			} else if len(warnings) != 1 || !strings.Contains(warnings[0], test.want) {
				t.Fatalf("missing %q: %v", test.want, warnings)
			}
			after, err := os.ReadFile(path)
			if err != nil || string(after) != test.raw {
				t.Fatalf("list changed index: %q (%v)", after, err)
			}
		})
	}
}
