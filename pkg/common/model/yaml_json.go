package model

import (
	"encoding/json"
	"fmt"
	"math"
	"strconv"
	"strings"

	"go.yaml.in/yaml/v3"
)

type yamlJSONMode uint8

const (
	yamlSerialize yamlJSONMode = iota
	jsonDeserialize
	indexNormalize
)

type YAMLField struct{ Key, Value *yaml.Node }
type YAMLFields []YAMLField

func (fields YAMLFields) Get(name string) *yaml.Node {
	for i := len(fields) - 1; i >= 0; i-- {
		if key, ok := yamlStringKey(fields[i].Key); ok && key == name {
			return fields[i].Value
		}
	}
	return nil
}

func unknownFields(node *yaml.Node, known ...string) YAMLFields {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	excluded := map[string]bool{}
	for _, name := range known {
		excluded[name] = true
	}
	fields := YAMLFields{}
	for i := 0; i < len(node.Content); i += 2 {
		key, isString := yamlStringKey(node.Content[i])
		if isString && excluded[key] {
			continue
		}
		fields = append(fields, YAMLField{node.Content[i], node.Content[i+1]})
	}
	return fields
}

func (fields YAMLFields) validateFlattened() error {
	var walk func(*yaml.Node) error
	walk = func(node *yaml.Node) error {
		if node.Kind == yaml.AliasNode {
			return walk(node.Alias)
		}
		if node.Tag != "" && !strings.HasPrefix(node.Tag, "!!") {
			return fmt.Errorf("flattened metadata does not support enum input at line %d", node.Line)
		}
		for _, child := range node.Content {
			if err := walk(child); err != nil {
				return err
			}
		}
		return nil
	}
	for _, field := range fields {
		if err := walk(field.Key); err != nil {
			return err
		}
		if err := walk(field.Value); err != nil {
			return err
		}
	}
	return nil
}

type yamlJSONValue struct {
	node *yaml.Node
	mode yamlJSONMode
}

func (value yamlJSONValue) MarshalJSON() ([]byte, error) {
	result, err := convertYAMLJSON(value.node, value.mode)
	if err != nil {
		return nil, err
	}
	return json.Marshal(result)
}
func (value yamlJSONValue) MarshalYAML() (any, error) {
	if value.mode == indexNormalize {
		result, err := convertYAMLJSON(value.node, value.mode)
		if err != nil {
			return nil, err
		}
		return nativeJSON(result), nil
	}
	return expandedYAML(value.node)
}
func yamlValue(node *yaml.Node) any { return yamlJSONValue{node: node, mode: yamlSerialize} }

// JSONFromYAML applies the JSON-value deserialization contract to a raw YAML
// section without imposing any domain record schema on authoritative rows.
func JSONFromYAML(node *yaml.Node) (any, error) {
	if err := validateNodeGraph(node); err != nil {
		return nil, err
	}
	return convertYAMLJSON(node, jsonDeserialize)
}

type jsonValue struct{ value any }

func (value jsonValue) MarshalJSON() ([]byte, error) { return json.Marshal(value.value) }
func (value jsonValue) MarshalYAML() (any, error)    { return value.value, nil }

func nativeJSON(value any) any {
	switch value := value.(type) {
	case jsonValue:
		return nativeJSON(value.value)
	case json.RawMessage:
		var result any
		if err := json.Unmarshal(value, &result); err == nil {
			return result
		}
		return nil
	case []any:
		result := make([]any, len(value))
		for i, child := range value {
			result[i] = nativeJSON(child)
		}
		return result
	case map[string]any:
		result := make(map[string]any, len(value))
		for key, child := range value {
			result[key] = nativeJSON(child)
		}
		return result
	default:
		return value
	}
}

func serdeFloat(value float64) string {
	if math.IsNaN(value) || math.IsInf(value, 0) {
		return "null"
	}
	scientific := strconv.FormatFloat(value, 'e', -1, 64)
	parts := strings.Split(scientific, "e")
	exponent, _ := strconv.Atoi(parts[1])
	if exponent >= -5 && exponent <= 15 {
		fixed := strconv.FormatFloat(value, 'f', -1, 64)
		if !strings.Contains(fixed, ".") {
			fixed += ".0"
		}
		return fixed
	}
	sign := ""
	if exponent >= 0 {
		sign = "+"
	}
	return parts[0] + "e" + sign + strconv.Itoa(exponent)
}

func yamlJSONKey(node *yaml.Node, mode yamlJSONMode) (string, error) {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	if key, ok := yamlStringKey(node); ok {
		return key, nil
	}
	if mode == indexNormalize {
		return "", nil
	}
	if node.Kind != yaml.ScalarNode {
		return "", fmt.Errorf("key must be a string")
	}
	if mode == jsonDeserialize {
		return node.Value, nil
	}
	var value any
	if err := node.Decode(&value); err != nil {
		return "", err
	}
	switch value := value.(type) {
	case int:
		return strconv.Itoa(value), nil
	case int64:
		return strconv.FormatInt(value, 10), nil
	case uint64:
		return strconv.FormatUint(value, 10), nil
	case bool:
		return strconv.FormatBool(value), nil
	case float64:
		if math.IsNaN(value) || math.IsInf(value, 0) {
			return "", fmt.Errorf("float key must be finite")
		}
		return serdeFloat(value), nil
	}
	return "", fmt.Errorf("key must be a string")
}

func convertYAMLJSON(node *yaml.Node, mode yamlJSONMode) (any, error) {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	if node.Tag != "" && !strings.HasPrefix(node.Tag, "!!") {
		if mode == indexNormalize {
			return nil, nil
		}
		if mode == jsonDeserialize {
			return nil, fmt.Errorf("enum input is not a JSON value")
		}
		copy := *node
		copy.Tag = ""
		copy.Tag = copy.ShortTag()
		value, err := convertYAMLJSON(&copy, mode)
		if err != nil {
			return nil, err
		}
		tag := node.Tag
		if !strings.HasPrefix(tag, "!") {
			tag = "!" + tag
		}
		return map[string]any{tag: value}, nil
	}
	switch node.Kind {
	case yaml.MappingNode:
		result := map[string]any{}
		for i := 0; i < len(node.Content); i += 2 {
			key, err := yamlJSONKey(node.Content[i], mode)
			if err != nil {
				return nil, err
			}
			value, err := convertYAMLJSON(node.Content[i+1], mode)
			if err != nil {
				return nil, err
			}
			result[key] = value
		}
		return result, nil
	case yaml.SequenceNode:
		result := []any{}
		for _, child := range node.Content {
			value, err := convertYAMLJSON(child, mode)
			if err != nil {
				return nil, err
			}
			result = append(result, value)
		}
		return result, nil
	case yaml.ScalarNode:
		if node.Tag == "!!str" || node.Tag == "!!timestamp" || node.Tag == "!!binary" {
			return node.Value, nil
		}
		var value any
		if err := node.Decode(&value); err != nil {
			return nil, err
		}
		if number, ok := value.(uint64); ok && mode == indexNormalize && number > math.MaxInt64 {
			value = float64(number)
		}
		if number, ok := value.(float64); ok {
			if math.IsNaN(number) || math.IsInf(number, 0) {
				if mode == indexNormalize {
					return 0, nil
				}
				return nil, nil
			}
			return json.RawMessage(serdeFloat(number)), nil
		}
		return value, nil
	}
	return nil, nil
}

func flattenJSON(fields map[string]any, extra YAMLFields) ([]byte, error) {
	for _, field := range extra {
		key, err := yamlJSONKey(field.Key, yamlSerialize)
		if err != nil {
			return nil, err
		}
		fields[key] = yamlValue(field.Value)
	}
	return json.Marshal(fields)
}

func flattenYAML(value any, extra YAMLFields) (*yaml.Node, error) {
	var node yaml.Node
	if err := node.Encode(value); err != nil {
		return nil, err
	}
	for _, field := range extra {
		key, err := expandedYAML(field.Key)
		if err != nil {
			return nil, err
		}
		value, err := expandedYAML(field.Value)
		if err != nil {
			return nil, err
		}
		node.Content = append(node.Content, key, value)
	}
	return &node, nil
}

// CloneYAMLValue expands aliases into an independent value before a partial
// metadata edit, so modifying an alias never changes its original anchor.
func CloneYAMLValue(node *yaml.Node) (*yaml.Node, error) { return expandedYAML(node) }

func expandedYAML(node *yaml.Node) (*yaml.Node, error) {
	if err := validateNodeGraph(node); err != nil {
		return nil, err
	}
	copies := map[*yaml.Node]*yaml.Node{}
	var clone func(*yaml.Node) *yaml.Node
	clone = func(node *yaml.Node) *yaml.Node {
		if node.Kind == yaml.AliasNode {
			return clone(node.Alias)
		}
		if copy := copies[node]; copy != nil {
			return copy
		}
		copy := *node
		copy.Anchor = ""
		copy.Alias = nil
		copies[node] = &copy
		if copy.Tag == "!!merge" || copy.Tag == "!!binary" {
			copy.Tag = "!!str"
		}
		copy.Content = make([]*yaml.Node, len(node.Content))
		for i, child := range node.Content {
			copy.Content[i] = clone(child)
		}
		return &copy
	}
	return clone(node), nil
}
