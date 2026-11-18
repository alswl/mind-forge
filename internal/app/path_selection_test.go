package app

import (
	"os"
	"path/filepath"
	"testing"
)

func TestExistingPathSelectsProjectWithoutChangingBareOrExplicitSelectors(t *testing.T) {
	root := canonical(t.TempDir())
	project := filepath.Join(root, "projects/lab")
	if err := os.MkdirAll(filepath.Join(project, "docs/post"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "mind.yaml"), []byte("schema: '1'\n"), 0644); err != nil {
		t.Fatal(err)
	}
	for _, selector := range []string{"projects/lab/docs/post", filepath.Join(project, "docs/post")} {
		selected, relative, ok := SelectProjectFromPath(root, root, selector, false)
		if !ok || selected != project || relative != "docs/post" {
			t.Fatalf("%s => %s %s %v", selector, selected, relative, ok)
		}
	}
	for _, test := range []struct {
		input    string
		explicit bool
	}{{"projects/lab/docs/post", true}, {"post", false}, {"projects/lab/../lab/docs/post", false}, {"projects/lab/absent", false}, {".", false}} {
		if _, _, ok := SelectProjectFromPath(root, root, test.input, test.explicit); ok {
			t.Fatalf("adopted %s explicit%v", test.input, test.explicit)
		}
	}
	selected, relative, ok := SelectProjectFromPath(root, filepath.Join(project, "docs/post"), ".", false)
	if !ok || selected != project || relative != "docs/post" {
		t.Fatalf("cwd directory %s %s %v", selected, relative, ok)
	}
}
