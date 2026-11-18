package output

import "github.com/alswl/mind-forge/pkg/common/outcome"

// Error is the shared contract; terminal presentation remains private.
type Error = outcome.Error

func Internal(cause error) *Error                { return outcome.Internal(cause) }
func NotInRepo() *Error                          { return outcome.NotInRepo() }
func Usage(message, hint string) *Error          { return outcome.Usage(message, hint) }
func ParseError(path, detail string) *Error      { return outcome.ParseError(path, detail) }
func NotImplemented(message, hint string) *Error { return outcome.NotImplemented(message, hint) }
