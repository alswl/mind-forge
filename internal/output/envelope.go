package output

import "github.com/alswl/mind-forge/pkg/common/outcome"

func Success(command string, data any) ([]byte, error) { return outcome.Success(command, data) }
func marshalLine(value any) ([]byte, error)            { return outcome.MarshalLine(value) }
func CompactJSON(value any) ([]byte, error)            { return outcome.CompactJSON(value) }
