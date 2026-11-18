package main

import (
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func TestProjectTermUpdateCorrectionErrorKeepsGuidance(t *testing.T) {
	hint := "use `mf term correction list`"
	missing := &output.Error{Kind: "not_found", Message: "correction 'global-only' not found on term 'RAG'", Hint: &hint}
	got := termUpdateError("project", missing).(*output.Error)
	if got.Kind != "usage" || got.ExitCode() != 2 || got.Message != missing.Message || got.Hint == nil || *got.Hint != hint {
		t.Fatalf("project error: %+v", got)
	}
	if missing.Kind != "not_found" {
		t.Fatal("mutated shared error")
	}
	if termUpdateError("global", missing) != missing {
		t.Fatal("changed global correction policy")
	}
	if termUpdateError("project", nil) != nil {
		t.Fatal("nil error changed")
	}
}
