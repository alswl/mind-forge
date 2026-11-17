package model

import (
	"encoding/json"
	"fmt"

	"go.yaml.in/yaml/v3"
)

func ValidateCorrections(terms []Term) error {
	for _, term := range terms {
		for _, correction := range term.Corrections {
			asciiWord := true
			for _, char := range correction.Original {
				if !(char >= 'A' && char <= 'Z' || char >= 'a' && char <= 'z' || char >= '0' && char <= '9' || char == '_' || char == '-') {
					asciiWord = false
				}
			}
			if asciiWord && correction.Match != "pinyin" && correction.Boundary != "loose" && len(correction.Original) > 0 {
				first, last := correction.Original[0], correction.Original[len(correction.Original)-1]
				if first == '-' || first == '_' || last == '-' || last == '_' {
					return fmt.Errorf("boundary: standalone cannot apply to identifier-character edges (correction '%s')", correction.Original)
				}
			}
		}
	}
	return nil
}

type Correction struct {
	Original string  `json:"original" yaml:"original"`
	Correct  string  `json:"correct" yaml:"correct"`
	Match    string  `json:"match,omitempty" yaml:"match,omitempty"`
	Fix      string  `json:"fix,omitempty" yaml:"fix,omitempty"`
	Boundary string  `json:"boundary,omitempty" yaml:"boundary,omitempty"`
	Pinyin   *string `json:"pinyin,omitempty" yaml:"pinyin,omitempty"`
}

func (correction *Correction) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "original", "correct"); err != nil {
		return err
	}
	if err := rejectNull(fields, "match", "fix", "boundary"); err != nil {
		return err
	}
	type wire Correction
	result := wire{Match: "word", Fix: "required", Boundary: "standalone"}
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	if err := enumValue(result.Match, "word", "substring", "pinyin"); err != nil {
		return err
	}
	if err := enumValue(result.Fix, "required", "suggested"); err != nil {
		return err
	}
	if err := enumValue(result.Boundary, "loose", "standalone"); err != nil {
		return err
	}
	*correction = Correction(result)
	return nil
}

func (correction Correction) compact() Correction {
	if correction.Match == "word" {
		correction.Match = ""
	}
	if correction.Fix == "required" {
		correction.Fix = ""
	}
	if correction.Boundary == "standalone" {
		correction.Boundary = ""
	}
	return correction
}

func (correction Correction) MarshalJSON() ([]byte, error) {
	type wire Correction
	return json.Marshal(wire(correction.compact()))
}

func (correction Correction) MarshalYAML() (any, error) {
	type wire Correction
	return wire(correction.compact()), nil
}

type Term struct {
	Term        string                `json:"term" yaml:"term"`
	Definition  *string               `json:"definition" yaml:"definition"`
	Description *string               `json:"description,omitempty" yaml:"description,omitempty"`
	Confidence  *float64              `json:"confidence,omitempty" yaml:"confidence,omitempty"`
	Aliases     JSONArray[string]     `json:"aliases" yaml:"aliases"`
	Tags        JSONArray[string]     `json:"tags" yaml:"tags"`
	Corrections JSONArray[Correction] `json:"corrections" yaml:"corrections"`
	Extra       []ExtraField          `json:"-" yaml:"-"`
}

func (term *Term) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "term"); err != nil {
		return err
	}
	if err := rejectNull(fields, "aliases", "tags", "corrections"); err != nil {
		return err
	}
	for _, name := range []string{"aliases", "tags", "corrections"} {
		if err := rejectNullItems(fields[name]); err != nil {
			return err
		}
	}
	type wire Term
	result := wire{Aliases: []string{}, Tags: []string{}, Corrections: []Correction{}}
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	for i := 0; i < len(node.Content); i += 2 {
		key := node.Content[i].Value
		switch key {
		case "term", "definition", "description", "confidence", "aliases", "tags", "corrections":
			continue
		}
		value, err := CloneYAMLValue(node.Content[i+1])
		if err != nil {
			return err
		}
		result.Extra = append(result.Extra, ExtraField{Key: key, Value: value})
	}
	*term = Term(result)
	return nil
}

func (term Term) MarshalYAML() (any, error) {
	type wire Term
	var node yaml.Node
	if err := node.Encode(wire(term)); err != nil {
		return nil, err
	}
	for _, field := range term.Extra {
		var value yaml.Node
		if err := value.Encode(field.Value); err != nil {
			return nil, err
		}
		node.Content = append(node.Content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: field.Key}, &value)
	}
	return &node, nil
}
