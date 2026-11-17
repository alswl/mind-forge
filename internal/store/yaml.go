package store

import (
	"bytes"
	"fmt"
	"io"

	"go.yaml.in/yaml/v3"
)

// Document retains the original text because decoding and re-encoding a YAML
// node changes quoting, comments and indentation outside the edited section.
type Document struct {
	raw  []byte
	Root yaml.Node
}

func ParseYAML(raw []byte) (*Document, error) {
	document := &Document{raw: bytes.Clone(raw)}
	decoder := yaml.NewDecoder(bytes.NewReader(raw))
	if err := decoder.Decode(&document.Root); err != nil && err != io.EOF {
		return nil, err
	}
	var next yaml.Node
	if err := decoder.Decode(&next); err != io.EOF {
		if err != nil {
			return nil, err
		}
		return nil, fmt.Errorf("expected one YAML document")
	}
	return document, nil
}

func (d *Document) Bytes() []byte {
	return bytes.Clone(d.raw)
}

func (d *Document) Decode(value any) error {
	return d.Root.Decode(value)
}

func (d *Document) DecodeMapping(value any) error {
	if len(d.Root.Content) != 1 || d.Root.Content[0].Kind != yaml.MappingNode {
		return fmt.Errorf("expected a YAML mapping")
	}
	return d.Decode(value)
}

func (d *Document) Section(key string) ([]byte, bool) {
	start, end, exists := topLevelSpan(d.raw, key)
	if !exists {
		return nil, false
	}
	return bytes.Clone(d.raw[start:end]), true
}

func (d *Document) ReplaceSection(key string, rendered []byte) []byte {
	start, end, exists := topLevelSpan(d.raw, key)
	if !exists {
		result := bytes.Clone(d.raw)
		if len(result) > 0 && result[len(result)-1] != '\n' {
			result = append(result, '\n')
		}
		return append(result, rendered...)
	}
	result := make([]byte, 0, start+len(rendered)+len(d.raw)-end)
	result = append(result, d.raw[:start]...)
	result = append(result, rendered...)
	return append(result, d.raw[end:]...)
}

// ReplaceProtectedSection materializes surviving aliases that depend on an
// anchor in the replaced section before that anchor is removed.
func (d *Document) ReplaceProtectedSection(key string, rendered []byte) ([]byte, error) {
	return replaceProtectedSection(d.raw, key, rendered)
}
