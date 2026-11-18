package store

import (
	"errors"
	"github.com/alswl/mind-forge/internal/output"
	"regexp"
	"testing"
)

func TestOriginalErrorVariants(t *testing.T) {
	t.Run("IncompatibleSchema", func(t *testing.T) {
		err := ValidateSchema("2", "/tmp/x.yaml")
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "incompatible_schema" || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(failure.Kind) {
			t.Fatal(err)
		}
		if failure.Hint == nil {
			t.Fatal("missing schema hint")
		}
	})
}
