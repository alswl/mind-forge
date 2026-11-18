package source

import (
	"errors"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestLegacySourceErrors(t *testing.T) {
	for _, test := range []struct {
		name string
		err  error
		kind string
	}{
		{"file_exists_kind_is_file_exists", fileExists("synthetic.md"), "file_exists"},
		{"not_found_kind_is_not_found", notFound("missing"), "not_found"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var failure *output.Error
			if !errors.As(test.err, &failure) || failure.Kind != test.kind {
				t.Fatalf("error = %v, want kind %s", test.err, test.kind)
			}
		})
	}
}
