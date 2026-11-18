package term

import (
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"go.yaml.in/yaml/v3"
)

type Scope struct{ Root, Kind string }

func (s Scope) Path() string {
	if s.Kind == "global" {
		return filepath.Join(s.Root, "minds-terms.yaml")
	}
	return filepath.Join(s.Root, "mind-index.yaml")
}

type termDocument struct {
	terms    []model.Term
	index    *store.IndexDocument
	global   *store.Document
	original []byte
	existed  bool
}

func (s Scope) document(strict bool) (*termDocument, error) {
	d := &termDocument{terms: []model.Term{}}
	raw, err := os.ReadFile(s.Path())
	if errors.Is(err, os.ErrNotExist) {
		raw = []byte("schema_version: '1'\n")
		if s.Kind != "global" {
			raw = []byte("schema: '1'\n")
		}
	} else if err != nil {
		return nil, err
	} else {
		d.original = raw
		d.existed = true
	}
	if s.Kind != "global" {
		index, err := store.DecodeIndex(raw, s.Path(), strict)
		if err != nil {
			return nil, err
		}
		d.index = index
		if index.Value.Terms != nil {
			d.terms = *index.Value.Terms
		}
		return d, nil
	}
	doc, err := store.ParseYAML(raw)
	if err != nil {
		return nil, output.ParseError(s.Path(), err.Error())
	}
	if len(doc.Root.Content) != 1 || doc.Root.Content[0].Kind != yaml.MappingNode {
		return nil, output.ParseError(s.Path(), "expected a YAML mapping for global terms")
	}
	root := doc.Root.Content[0]
	schema := ""
	schemaCount := 0
	seen := map[string]bool{}
	for i := 0; i < len(root.Content); i += 2 {
		key, value := root.Content[i].Value, root.Content[i+1]
		if seen[key] {
			return nil, output.ParseError(s.Path(), "duplicate field `"+key+"`")
		}
		seen[key] = true
		for value.Kind == yaml.AliasNode {
			value = value.Alias
		}
		switch key {
		case "schema", "schema_version":
			schemaCount++
			if value.Tag != "!!str" {
				return nil, output.ParseError(s.Path(), "schema_version must be a string")
			}
			schema = value.Value
		case "terms":
			if value.Kind != yaml.SequenceNode {
				return nil, output.ParseError(s.Path(), "terms must be a sequence")
			}
			for position, record := range value.Content {
				for record.Kind == yaml.AliasNode {
					record = record.Alias
				}
				if record.Tag == "!!null" {
					return nil, output.ParseError(s.Path(), fmt.Sprintf("invalid terms[%d]: null record", position))
				}
				var tm model.Term
				if err := record.Decode(&tm); err != nil {
					return nil, output.ParseError(s.Path(), err.Error())
				}
				d.terms = append(d.terms, tm)
			}
		}
	}
	if schemaCount != 1 {
		return nil, output.ParseError(s.Path(), "expected exactly one schema_version or schema field")
	}
	if err := store.ValidateSchema(schema, s.Path()); err != nil {
		return nil, err
	}
	if strict {
		if err := model.ValidateCorrections(d.terms); err != nil {
			return nil, output.Usage(err.Error(), "")
		}
	}
	d.global = doc
	return d, nil
}

func (s Scope) Load(strict bool) ([]model.Term, error) {
	d, err := s.document(strict)
	if err != nil {
		return nil, err
	}
	return d.terms, nil
}

func (s Scope) write(d *termDocument, dry bool) error {
	if err := model.ValidateCorrections(d.terms); err != nil {
		return output.Usage(err.Error(), "")
	}
	return s.publish(d, dry)
}

func (s Scope) publish(d *termDocument, dry bool) error {
	raw, err := s.render(d, true)
	if err != nil || dry {
		return err
	}
	return (store.AtomicWriter{}).Write(s.Path(), raw, 0644)
}

func (s Scope) render(d *termDocument, sortTerms bool) ([]byte, error) {
	if sortTerms {
		slices.SortStableFunc(d.terms, func(a, b model.Term) int { return strings.Compare(a.Term, b.Term) })
	}
	if d.index != nil {
		updated := d.index.Value
		updated.Terms = &d.terms
		return d.index.Render(updated)
	}
	rendered, err := yaml.Marshal(struct {
		Terms []model.Term `yaml:"terms"`
	}{d.terms})
	if err != nil {
		return nil, err
	}
	return d.global.ReplaceProtectedSection("terms", rendered)
}

var errMissingTerm = errors.New("canonical term missing")

// IsMissingTerm distinguishes an absent canonical term from an absent correction.
func IsMissingTerm(err error) bool {
	return errors.Is(err, errMissingTerm)
}

func missingTerm(name string) error {
	hint := "use `mf term list` or `mf term new`"
	return &output.Error{Kind: "not_found", Message: fmt.Sprintf("term '%s' not found", name), Hint: &hint, Cause: errMissingTerm}
}

func findTerm(terms []model.Term, name string) (int, error) {
	for i, t := range terms {
		if t.Term == name {
			return i, nil
		}
	}
	return -1, missingTerm(name)
}

func (s Scope) Show(name string) (model.Term, error) {
	terms, err := s.Load(false)
	if err != nil {
		return model.Term{}, err
	}
	i, err := findTerm(terms, name)
	if err != nil {
		return model.Term{}, err
	}
	return terms[i], nil
}

type Input struct {
	Definition, Description *string
	Confidence              *float64
	Aliases, Tags           []string
}

type NewResult struct {
	Term                 model.Term
	Created              bool     `json:"created"`
	AddedAliases         []string `json:"added_aliases"`
	AddedTags            []string `json:"added_tags"`
	AddedMisrecognitions []string `json:"added_misrecognitions"`
}

func validateConfidence(value *float64) error {
	if value == nil {
		return nil
	}
	v := *value
	if math.IsNaN(v) || math.IsInf(v, 0) {
		return output.Usage("confidence must be a finite number between 0.0 and 1.0", "")
	}
	if v < 0 || v > 1 {
		return output.Usage(fmt.Sprintf("confidence %g is out of range; valid range is 0.0 to 1.0", v), "choose a value between 0.0 and 1.0")
	}
	return nil
}

func dedup(values []string) []string {
	result := []string{}
	for _, v := range values {
		if !slices.Contains(result, v) {
			result = append(result, v)
		}
	}
	return result
}

func (s Scope) New(name string, input Input, misrecognitions []string, dry bool) (NewResult, error) {
	result := NewResult{AddedAliases: []string{}, AddedTags: []string{}, AddedMisrecognitions: []string{}}
	if strings.TrimSpace(name) == "" {
		return result, output.Usage("term name cannot be empty", "")
	}
	if err := validateConfidence(input.Confidence); err != nil {
		return result, err
	}
	d, err := s.document(true)
	if err != nil {
		return result, err
	}
	position := -1
	for i, t := range d.terms {
		if t.Term == name {
			position = i
			break
		}
	}
	aliases, tags, misrecognitions := dedup(input.Aliases), dedup(input.Tags), dedup(misrecognitions)
	for _, alias := range aliases {
		if position >= 0 && (alias == name || slices.Contains(d.terms[position].Aliases, alias)) {
			continue
		}
		for _, other := range d.terms {
			if other.Term != name && (other.Term == alias || slices.Contains(other.Aliases, alias)) {
				return result, &output.Error{Kind: "not_found", Message: fmt.Sprintf("alias '%s' conflicts with existing term '%s'", alias, other.Term)}
			}
		}
	}
	if position < 0 {
		tm := model.Term{Term: name, Definition: input.Definition, Description: input.Description, Confidence: input.Confidence, Aliases: aliases, Tags: tags, Corrections: []model.Correction{}}
		if tm.Definition != nil && *tm.Definition == "" {
			tm.Definition = nil
		}
		for _, raw := range misrecognitions {
			original, correct, ok := strings.Cut(raw, ":")
			if !ok {
				correct = name
			}
			tm.Corrections = append(tm.Corrections, model.Correction{Original: original, Correct: correct})
		}
		for _, alias := range aliases {
			if !slices.ContainsFunc(tm.Corrections, func(c model.Correction) bool { return c.Original == alias }) {
				tm.Corrections = append(tm.Corrections, model.Correction{Original: alias, Correct: name})
			}
		}
		d.terms = append(d.terms, tm)
		result.Term = tm
		result.Created = true
		result.AddedAliases = aliases
		result.AddedTags = tags
		result.AddedMisrecognitions = misrecognitions
	} else {
		tm := &d.terms[position]
		for _, alias := range aliases {
			if alias == name || slices.Contains(tm.Aliases, alias) {
				continue
			}
			tm.Aliases = append(tm.Aliases, alias)
			result.AddedAliases = append(result.AddedAliases, alias)
			if !slices.ContainsFunc(tm.Corrections, func(c model.Correction) bool { return c.Original == alias }) {
				tm.Corrections = append(tm.Corrections, model.Correction{Original: alias, Correct: name})
			}
		}
		for _, tag := range tags {
			if !slices.Contains(tm.Tags, tag) {
				tm.Tags = append(tm.Tags, tag)
				result.AddedTags = append(result.AddedTags, tag)
			}
		}
		for _, raw := range misrecognitions {
			original, correct, ok := strings.Cut(raw, ":")
			if !ok || slices.ContainsFunc(tm.Corrections, func(c model.Correction) bool { return c.Original == original && c.Correct == correct }) {
				continue
			}
			tm.Corrections = append(tm.Corrections, model.Correction{Original: original, Correct: correct})
			result.AddedMisrecognitions = append(result.AddedMisrecognitions, raw)
		}
		result.Term = *tm
	}
	// Registration retains the reference's raw misrecognition values. Later
	// strict reads enforce cross-field invariants; repair paths load leniently.
	return result, s.publish(d, dry)
}
