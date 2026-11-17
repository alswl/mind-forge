package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOutputProjectionPreservesUnrelatedIndexSections(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "mind.yaml", "schema: '1'\ncontent_layout: category\nminds: {idea: {}}\n")
	putContent(t, root, "outputs/idea/summary.md", "# Public\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\n# keep-author-registration\nadditional:\n- identity: kept\n  custom: value\nopaque: {marker: survives}\noutputs: []\n")
	if err := IndexOutputProjection(root, root); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if err != nil {
		t.Fatal(err)
	}
	for _, expected := range []string{"# keep-author-registration", "identity: kept", "custom: value", "opaque: {marker: survives}", "outputs/idea/summary.md"} {
		if !strings.Contains(string(raw), expected) {
			t.Fatalf("missing %q: %s", expected, raw)
		}
	}
}
