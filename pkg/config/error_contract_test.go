package config

import (
	"errors"
	"os"
	"path/filepath"
	"regexp"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestOriginalErrorVariants(t *testing.T) {
	t.Run("InvalidTemplateName", func(t *testing.T) {
		path := filepath.Join(t.TempDir(), "mind.yaml")
		if err := os.WriteFile(path, []byte("schema: '1'\ntemplates:\n  Bad: {pattern: 'docs/{date:YYYY}.md', mode: file}\n"), 0o600); err != nil {
			t.Fatal(err)
		}
		_, err := readProjectConfig(path)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "invalid_template_name" || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(failure.Kind) {
			t.Fatal(err)
		}
		if failure.Hint == nil {
			t.Fatal("template hint missing")
		}
	})
}
