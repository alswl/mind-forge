package source

import (
	"errors"
	"github.com/alswl/mind-forge/internal/output"
	"regexp"
	"testing"
)

func TestOriginalErrorVariants(t *testing.T) {
	for _, row := range []struct {
		name string
		err  error
		kind string
	}{{"FileExists", fileExists("/tmp/x"), "file_exists"}, {"NotFound", notFound("x"), "not_found"}} {
		t.Run(row.name, func(t *testing.T) {
			var failure *output.Error
			if !errors.As(row.err, &failure) || failure.Kind != row.kind || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(failure.Kind) {
				t.Fatal(row.err)
			}
			if failure.Hint == nil {
				t.Fatalf("hint absent for %s", row.name)
			}
		})
	}
}
