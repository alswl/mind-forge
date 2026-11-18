package content

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestLegacyMindDiscoverySafety(t *testing.T) {
	t.Run("bundle_excludes_configured_and_hidden_roots_and_preserves_zero_minds", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "mind.yaml", "schema: '1'\ncontent_layout: bundle\nlayout:\n  templates: guides\n  build_output: generated\nminds:\n  registered: {}\n")
		for _, name := range []string{"guides", "generated", ".tool", "build", "ordinary"} {
			if err := os.Mkdir(filepath.Join(project, name), 0700); err != nil {
				t.Fatal(err)
			}
		}
		minds, err := Discover(project, project)
		if err != nil {
			t.Fatal(err)
		}
		names := []string{}
		for _, mind := range minds {
			names = append(names, mind.Name)
			if len(mind.Outputs) != 0 {
				t.Fatalf("invented outputs: %+v", mind)
			}
		}
		if !reflect.DeepEqual(names, []string{"ordinary", "registered"}) {
			t.Fatalf("mind names = %v", names)
		}
		if minds[1].Path != nil {
			t.Fatalf("registered-only mind has path = %q", *minds[1].Path)
		}
	})
	t.Run("output_shape_case_and_symlink_collisions_are_not_guessed", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "mind.yaml", "content_layout: category\nminds:\n  brief: {}\n")
		putContent(t, project, "outputs/brief/summary.md", "# Summary")
		if err := os.Mkdir(filepath.Join(project, "outputs/brief/SUMMARY"), 0700); err != nil {
			t.Fatal(err)
		}
		_, err := Discover(project, project)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "shape_conflict" {
			t.Fatalf("shape/case collision = %v", err)
		}
	})
	t.Run("symlink_blocks_and_build_parents_are_rejected", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "mind.yaml", "content_layout: bundle\n")
		if err := os.MkdirAll(filepath.Join(project, "brief/outputs/design"), 0700); err != nil {
			t.Fatal(err)
		}
		putContent(t, project, "secret.md", "secret")
		if err := os.Symlink(filepath.Join(project, "secret.md"), filepath.Join(project, "brief/outputs/design/01-title.md")); err != nil {
			t.Fatal(err)
		}
		if _, err := Discover(project, project); err == nil {
			t.Error("discovery accepted a symlink block")
		}
		if err := os.Symlink(filepath.Join(project, "brief"), filepath.Join(project, "build")); err != nil {
			t.Fatal(err)
		}
		if _, err := CheckedPath(project, "build/design.md"); err == nil {
			t.Error("checked path accepted a symlink build parent")
		}
	})
}
