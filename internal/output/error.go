package output

import (
	"errors"
	"fmt"
	"os"
	"strings"
	"syscall"
)

type Error struct {
	Kind       string
	Message    string
	Hint       *string
	Candidates *[]string
	Lifecycle  bool
	Cause      error
}

func (err *Error) Error() string {
	return err.Message
}

func (err *Error) Unwrap() error {
	return err.Cause
}

func (err *Error) ExitCode() int {
	if err.Lifecycle {
		return 2
	}
	switch err.Kind {
	case "usage", "selection_ambiguous", "content_conflict", "incompatible_schema", "unknown_template":
		return 2
	case "not_implemented":
		return 64
	default:
		return 1
	}
}

func (err *Error) JSON() []byte {
	body := struct {
		Kind       string    `json:"kind"`
		Message    string    `json:"message"`
		Hint       *string   `json:"hint"`
		Candidates *[]string `json:"candidates,omitempty"`
	}{err.Kind, err.Message, err.Hint, err.Candidates}
	raw, _ := marshalLine(struct {
		Status  string `json:"status"`
		Command string `json:"command"`
		Error   any    `json:"error"`
	}{"error", "mf", body})
	return raw
}

// Internal preserves the reference failure category for unclassified runtime
// errors. Direct OS wrappers carry Go operation/path prefixes absent in Rust.
func Internal(cause error) *Error {
	message := cause.Error()
	var native error
	switch failure := cause.(type) {
	case *os.PathError:
		native = failure.Err
	case *os.LinkError:
		native = failure.Err
	case *os.SyscallError:
		native = failure.Err
	case syscall.Errno:
		native = failure
	}
	var number syscall.Errno
	if native != nil && errors.As(native, &number) {
		description := number.Error()
		if description != "" {
			description = strings.ToUpper(description[:1]) + description[1:]
		}
		message = fmt.Sprintf("%s (os error %d)", description, number)
	}
	hint := "this is an internal error; please report it"
	return &Error{Kind: "internal", Message: message, Hint: &hint, Cause: cause}
}

func NotInRepo() *Error {
	hint := "run from a directory under a mind repo, or pass `--root <mind-repo>`"
	return &Error{Kind: "not_in_mind_repo", Message: "not in a mind repo", Hint: &hint}
}

func Usage(message string, hint string) *Error {
	err := &Error{Kind: "usage", Message: message}
	if hint != "" {
		err.Hint = &hint
	}
	return err
}

func ParseError(path, detail string) *Error {
	hint := "check the file format and try again"
	return &Error{Kind: "parse_error", Message: "yaml parse error in " + path + ": " + strings.TrimSpace(detail), Hint: &hint}
}
