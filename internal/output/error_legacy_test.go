package output

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"syscall"
	"testing"
)

func TestLegacyErrorConstructors(t *testing.T) {
	var invalid any
	jsonError := json.Unmarshal(nil, &invalid)
	for _, test := range []struct {
		name string
		err  *Error
		kind string
		hint bool
	}{
		{"usage_kind_is_usage", Usage("bad input", ""), "usage", false},
		{"io_kind_is_io", Internal(&os.PathError{Op: "open", Path: "synthetic", Err: syscall.ENOENT}), "internal", true},
		{"internal_kind_is_internal", Internal(errors.New("unexpected error")), "internal", true},
		{"json_kind_is_internal", Internal(jsonError), "internal", true},
		{"not_in_mind_repo_kind_is_not_in_mind_repo", NotInRepo(), "not_in_mind_repo", true},
		{"parse_error_kind_is_parse_error", ParseError("synthetic.yaml", "syntax error"), "parse_error", true},
		{"internal_hint_is_some", Internal(errors.New("test")), "internal", true},
		{"io_hint_is_some", Internal(&os.PathError{Op: "read", Path: "synthetic", Err: syscall.EIO}), "internal", true},
		{"json_hint_is_some", Internal(jsonError), "internal", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			if test.err.Kind != test.kind {
				t.Fatalf("error kind = %q, want %q", test.err.Kind, test.kind)
			}
			if test.hint && test.err.Hint == nil {
				t.Fatal("error hint absent")
			}
		})
	}
}

func TestInternalFilesystemMessageAndCause(t *testing.T) {
	for _, cause := range []error{
		&os.PathError{Op: "read", Path: "project/mind.yaml", Err: syscall.EISDIR},
		&os.LinkError{Op: "rename", Old: "old", New: "new", Err: syscall.EISDIR},
		os.NewSyscallError("read", syscall.EISDIR), syscall.EISDIR,
	} {
		failure := Internal(cause)
		if failure.Message != "Is a directory (os error 21)" || failure.Hint == nil || *failure.Hint != "this is an internal error; please report it" || failure.ExitCode() != 1 {
			t.Fatalf("raw OS failure changed: %+v", failure)
		}
		if !errors.Is(failure, cause) {
			t.Fatal("failure lost original cause")
		}
	}
	cause := fmt.Errorf("context retained: %w", errors.New("failure"))
	if got := Internal(cause); got.Message != cause.Error() || !errors.Is(got, cause) {
		t.Fatalf("contextual failure rewritten: %+v", got)
	}
}
