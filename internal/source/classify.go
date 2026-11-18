package source

import (
	"fmt"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
)

type InputForm string

const (
	PathInput InputForm = "path"
	URLInput  InputForm = "url"
)

func ClassifyInput(value string) InputForm {
	if strings.HasPrefix(value, "http://") || strings.HasPrefix(value, "https://") {
		return URLInput
	}
	return PathInput
}

// Registration syntax is intentionally separate from the HTTP client's URL
// validation, which runs only when acquisition is requested.
func ValidateURL(value string) error {
	rest, found := strings.CutPrefix(value, "https://")
	if !found {
		rest, found = strings.CutPrefix(value, "http://")
	}
	if !found || rest == "" {
		return output.Usage(fmt.Sprintf("invalid URL '%s': must start with http:// or https:// and include a host", value), "")
	}
	return nil
}

func NameCollisionError(name string, suggestion *string) error {
	hint := "choose a unique --name"
	if suggestion != nil {
		hint = "try --name " + *suggestion
	}
	return output.Usage("source name '"+name+"' is already registered", hint)
}
