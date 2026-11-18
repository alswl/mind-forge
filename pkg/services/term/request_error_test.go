package term

import (
	"errors"
	"strings"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestTermProjectBoundaryDiagnosticKeepsItsScope(t *testing.T) {
	original := output.Usage("path is outside the boundary root", "use a path under the repo root")
	got := termProjectBoundaryError("/repo/alpha", "/repo/minds.yaml", original)
	var failure *output.Error
	if !errors.As(got, &failure) || failure.ExitCode() != 2 || !strings.Contains(failure.Message, "not under project root") || !strings.Contains(failure.Message, "/repo/alpha") || !strings.Contains(failure.Message, "/repo/minds.yaml") || failure.Hint != nil {
		t.Fatalf("project boundary error=%+v", failure)
	}
	if original.Hint == nil {
		t.Fatal("mutated shared error")
	}
	native := errors.New("permission denied")
	if termProjectBoundaryError("/repo/alpha", "/repo/minds.yaml", native) != native {
		t.Fatal("changed non-boundary filesystem failure")
	}
	if termProjectBoundaryError("/repo/alpha", "/repo/minds.yaml", nil) != nil {
		t.Fatal("changed successful resolution")
	}
}
