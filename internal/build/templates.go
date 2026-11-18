package build

import (
	"fmt"
	"os"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
)

func readText(path string) (string, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if !utf8.Valid(raw) {
		return "", fmt.Errorf("%s: stream did not contain valid UTF-8", path)
	}
	return string(raw), nil
}

func loadHTMLTemplate(project string, configured *string) (string, error) {
	value := DefaultHTMLTemplate
	if configured != nil {
		path, err := content.CheckedPath(project, *configured)
		if err != nil {
			return "", err
		}
		value, err = readText(path)
		if err != nil {
			return "", err
		}
	}
	value, err := PrepareHTMLTemplate(value)
	if err != nil {
		return "", &output.Error{Kind: "invalid_html_template", Message: err.Error(), Lifecycle: true, Cause: err}
	}
	return value, nil
}
