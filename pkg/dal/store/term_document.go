package store

import (
	"errors"
	"fmt"
	"os"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"go.yaml.in/yaml/v3"
)

type TermDocument struct {
	Terms    []model.Term
	Index    *IndexDocument
	Original []byte
	Existed  bool
	global   *Document
}

func ReadTermDocument(path string, globalScope bool, validate func([]model.Term) error) (*TermDocument, error) {
	d := &TermDocument{Terms: []model.Term{}}
	raw, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		raw = []byte("schema_version: '1'\n")
		if !globalScope {
			raw = []byte("schema: '1'\n")
		}
	} else if err != nil {
		return nil, err
	} else {
		d.Original = raw
		d.Existed = true
	}
	if !globalScope {
		index, err := decodeIndex(raw, path, validate)
		if err != nil {
			return nil, err
		}
		d.Index = index
		if index.Value.Terms != nil {
			d.Terms = *index.Value.Terms
		}
		return d, nil
	}
	doc, err := ParseYAML(raw)
	if err != nil {
		return nil, output.ParseError(path, err.Error())
	}
	if len(doc.Root.Content) != 1 || doc.Root.Content[0].Kind != yaml.MappingNode {
		return nil, output.ParseError(path, "expected a YAML mapping for global terms")
	}
	root := doc.Root.Content[0]
	schema := ""
	schemaCount := 0
	seen := map[string]bool{}
	for i := 0; i < len(root.Content); i += 2 {
		key, value := root.Content[i].Value, root.Content[i+1]
		if seen[key] {
			return nil, output.ParseError(path, "duplicate field `"+key+"`")
		}
		seen[key] = true
		for value.Kind == yaml.AliasNode {
			value = value.Alias
		}
		switch key {
		case "schema", "schema_version":
			schemaCount++
			if value.Tag != "!!str" {
				return nil, output.ParseError(path, "schema_version must be a string")
			}
			schema = value.Value
		case "terms":
			if value.Kind != yaml.SequenceNode {
				return nil, output.ParseError(path, "terms must be a sequence")
			}
			for position, record := range value.Content {
				for record.Kind == yaml.AliasNode {
					record = record.Alias
				}
				if record.Tag == "!!null" {
					return nil, output.ParseError(path, fmt.Sprintf("invalid terms[%d]: null record", position))
				}
				var tm model.Term
				if err := record.Decode(&tm); err != nil {
					return nil, output.ParseError(path, err.Error())
				}
				d.Terms = append(d.Terms, tm)
			}
		}
	}
	if schemaCount != 1 {
		return nil, output.ParseError(path, "expected exactly one schema_version or schema field")
	}
	if err := ValidateSchema(schema, path); err != nil {
		return nil, err
	}
	if validate != nil {
		if err := validate(d.Terms); err != nil {
			return nil, err
		}
	}
	d.global = doc
	return d, nil
}

func (d *TermDocument) RenderTerms(terms []model.Term) ([]byte, error) {
	if d.Index != nil {
		updated := d.Index.Value
		updated.Terms = &terms
		return d.Index.Render(updated)
	}
	rendered, err := yaml.Marshal(struct {
		Terms []model.Term `yaml:"terms"`
	}{terms})
	if err != nil {
		return nil, err
	}
	return d.global.ReplaceProtectedSection("terms", rendered)
}

func WriteTermDocument(path string, raw []byte) error {
	return (AtomicWriter{}).Write(path, raw, 0644)
}
