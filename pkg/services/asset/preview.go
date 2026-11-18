package asset

import "path/filepath"

type NewPreview struct{ Name, Source string }

func PreviewNew(input string, name *string) NewPreview {
	value := filepath.Base(input)
	if value == "." || value == "/" {
		value = "unknown"
	}
	if name != nil {
		value = *name
	}
	return NewPreview{Name: value, Source: input}
}
