package app

import (
	"errors"
	"github.com/alswl/mind-forge/internal/output"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLegacyInvalidTemplateName(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind.yaml")
	if err := os.WriteFile(path, []byte("schema: '1'\ntemplates:\n  BadName: {pattern: 'docs/{date:YYYY}.md', mode: file}\n"), 0600); err != nil {
		t.Fatal(err)
	}
	_, err := readProjectConfig(path)
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "invalid_template_name" || failure.Hint == nil || !strings.Contains(*failure.Hint, "^[a-z][a-z0-9_]*$") {
		t.Fatalf("invalid template name = %v", err)
	}
}
