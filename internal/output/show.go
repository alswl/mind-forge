package output

import (
	"fmt"
	"path/filepath"
	"strings"
)

type DisplayOptions struct {
	RepoRoot   string
	Hyperlinks bool
}
type ShowBlock struct {
	Kind, Identity string
	Fields         []ShowField
	Sections       []ShowSection
}
type ShowField struct {
	Label string
	Value ShowValue
}
type ShowSection struct {
	Heading string
	Fields  []ShowField
}
type ShowValue struct {
	Text     string
	Optional bool
	Present  bool
	Path     bool
}

func RenderLink(visible, uri string, enabled bool) string {
	if !enabled {
		return visible
	}
	return "\x1b]8;;" + uri + "\x1b\\" + visible + "\x1b]8;;\x1b\\"
}

func RenderPathLink(path string, options DisplayOptions) string {
	if !options.Hyperlinks || options.RepoRoot == "" {
		return path
	}
	absolute := path
	if !filepath.IsAbs(path) {
		absolute = filepath.Join(options.RepoRoot, path)
	}
	var encoded strings.Builder
	for _, char := range []byte(absolute) {
		if char >= 'A' && char <= 'Z' || char >= 'a' && char <= 'z' || char >= '0' && char <= '9' || strings.ContainsRune("/-._~:@!$&'()*+,;=", rune(char)) {
			encoded.WriteByte(char)
		} else {
			fmt.Fprintf(&encoded, "%%%02X", char)
		}
	}
	return RenderLink(path, "file://"+encoded.String(), true)
}

func RenderShow(block ShowBlock, options DisplayOptions) string {
	width := 0
	measure := func(fields []ShowField) {
		for _, field := range fields {
			if size := DisplayWidth(field.Label + ":"); size > width {
				width = size
			}
		}
	}
	measure(block.Fields)
	for _, section := range block.Sections {
		measure(section.Fields)
	}
	var result strings.Builder
	write := func(fields []ShowField, indent int) {
		for _, field := range fields {
			label := field.Label + ":"
			result.WriteString(strings.Repeat(" ", indent) + label + strings.Repeat(" ", width-DisplayWidth(label)+2))
			value := field.Value.Text
			if field.Value.Optional && !field.Value.Present {
				value = "-"
			} else if field.Value.Path {
				value = RenderPathLink(value, options)
			}
			result.WriteString(value)
			result.WriteByte('\n')
		}
	}
	write(block.Fields, 0)
	for _, section := range block.Sections {
		if result.Len() != 0 {
			result.WriteByte('\n')
		}
		result.WriteString(section.Heading + ":\n")
		write(section.Fields, 2)
	}
	return result.String()
}

func ShowEnvelope(block ShowBlock, extra map[string]any) map[string]any {
	result := make(map[string]any, len(extra)+2)
	for key, value := range extra {
		result[key] = value
	}
	result["kind"], result["identity"] = block.Kind, block.Identity
	return result
}
