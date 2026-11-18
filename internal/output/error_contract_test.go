package output

import (
	"encoding/json"
	"errors"
	"os"
	"regexp"
	"testing"
)

func TestOriginalNotImplementedKind(t *testing.T) {
	failure := NotImplemented("feature x", "hint")
	if failure.Kind != "not_implemented" {
		t.Fatal(failure)
	}
	if failure.ExitCode() != 64 || failure.Hint == nil || *failure.Hint != "hint" {
		t.Fatal(failure)
	}
	if NotImplemented("feature x", "").Hint != nil {
		t.Fatal("empty-hint caller changed")
	}
}
func TestOriginalErrorVariants(t *testing.T) {
	var value any
	jsonFailure := json.Unmarshal([]byte(""), &value)
	for _, row := range []struct {
		name    string
		failure *Error
		kind    string
	}{
		{"Usage", Usage("test", "hint"), "usage"},
		{"Internal", Internal(errors.New("test")), "internal"},
		{"Io", Internal(&os.PathError{Op: "open", Path: "/tmp/x", Err: errors.New("test")}), "internal"},
		{"Json", Internal(jsonFailure), "internal"},
		{"NotInMindRepo", NotInRepo(), "not_in_mind_repo"},
		{"ParseError", ParseError("/tmp/x.yaml", "syntax"), "parse_error"},
		{"NotImplemented", NotImplemented("x", "hint"), "not_implemented"},
	} {
		t.Run(row.name, func(t *testing.T) {
			if row.failure.Kind != row.kind || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(row.failure.Kind) {
				t.Fatalf("invalid kind: %+v", row.failure)
			}
			if row.failure.Hint == nil {
				t.Fatalf("hint absent for %s", row.name)
			}
		})
	}
}
