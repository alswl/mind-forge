package model

import (
	"fmt"
	"reflect"
	"strings"

	"go.yaml.in/yaml/v3"
)

func mapping(node *yaml.Node) (map[string]*yaml.Node, error) {
	if err := validateNodeGraph(node); err != nil {
		return nil, err
	}
	if node.Kind == yaml.AliasNode {
		return mapping(node.Alias)
	}
	if node.Kind != yaml.MappingNode {
		return nil, fmt.Errorf("expected a YAML mapping")
	}
	fields := make(map[string]*yaml.Node, len(node.Content)/2)
	for index := 0; index < len(node.Content); index += 2 {
		key := node.Content[index]
		name, ok := yamlStringKey(key)
		if !ok {
			continue
		}
		if _, exists := fields[name]; exists {
			return nil, fmt.Errorf("duplicate field %q", name)
		}
		value := node.Content[index+1]
		for value.Kind == yaml.AliasNode {
			value = value.Alias
		}
		fields[name] = value
	}
	return fields, nil
}

func validateNodeGraph(root *yaml.Node) error {
	visited := make(map[*yaml.Node]uint8)
	var walk func(*yaml.Node) error
	walk = func(node *yaml.Node) error {
		if node == nil {
			return fmt.Errorf("invalid YAML alias")
		}
		if visited[node] == 1 {
			return fmt.Errorf("recursive YAML alias at line %d", node.Line)
		}
		if visited[node] == 2 {
			return nil
		}
		visited[node] = 1
		if node.Kind == yaml.AliasNode {
			if err := walk(node.Alias); err != nil {
				return err
			}
		}
		for _, child := range node.Content {
			if err := walk(child); err != nil {
				return err
			}
		}
		visited[node] = 2
		return nil
	}
	return walk(root)
}

func requireFields(fields map[string]*yaml.Node, names ...string) error {
	for _, name := range names {
		if node := fields[name]; node == nil || node.Tag == "!!null" {
			return fmt.Errorf("missing or null field %q", name)
		}
	}
	return nil
}

func rejectNull(fields map[string]*yaml.Node, names ...string) error {
	for _, name := range names {
		if node := fields[name]; node != nil && node.Tag == "!!null" {
			return fmt.Errorf("field %q cannot be null", name)
		}
	}
	return nil
}

func rejectNullItems(node *yaml.Node) error {
	if node == nil {
		return nil
	}
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	for position, item := range node.Content {
		if node.Kind == yaml.MappingNode && position%2 == 0 {
			continue
		}
		for item.Kind == yaml.AliasNode {
			item = item.Alias
		}
		if item.Tag == "!!null" {
			return fmt.Errorf("null collection entry at line %d", item.Line)
		}
	}
	return nil
}

func scalarString(value string) *yaml.Node {
	return &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: value}
}

func yamlStringKey(node *yaml.Node) (string, bool) {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	if node.Tag == "!!str" || node.Tag == "!!timestamp" || node.Tag == "!!merge" || node.Tag == "!!binary" {
		return node.Value, true
	}
	return "", false
}

// serde_yaml leaves merge keys as ordinary metadata. Decode only known record
// fields and disable go-yaml's implicit merge expansion without changing the AST.
func decodeYAMLRecord(node *yaml.Node, target any) error {
	if err := validateNodeGraph(node); err != nil {
		return err
	}
	copies := map[*yaml.Node]*yaml.Node{}
	var clone func(*yaml.Node) *yaml.Node
	clone = func(node *yaml.Node) *yaml.Node {
		if node == nil {
			return nil
		}
		if copy := copies[node]; copy != nil {
			return copy
		}
		copy := *node
		copies[node] = &copy
		if copy.Tag == "!!merge" || copy.Tag == "!!binary" {
			copy.Tag = "!!str"
		}
		copy.Content = make([]*yaml.Node, len(node.Content))
		for i, child := range node.Content {
			copy.Content[i] = clone(child)
		}
		copy.Alias = clone(node.Alias)
		return &copy
	}
	copy := clone(node)
	for copy.Kind == yaml.AliasNode {
		copy = copy.Alias
	}
	typ := reflect.TypeOf(target)
	if typ.Kind() == reflect.Pointer {
		typ = typ.Elem()
	}
	if copy.Kind == yaml.MappingNode && typ.Kind() == reflect.Struct {
		known := map[string]reflect.Type{}
		for i := 0; i < typ.NumField(); i++ {
			field := typ.Field(i)
			name := strings.Split(field.Tag.Get("yaml"), ",")[0]
			if name == "" {
				name = strings.ToLower(field.Name)
			}
			if name != "-" {
				known[name] = field.Type
			}
		}
		content := make([]*yaml.Node, 0, len(copy.Content))
		for i := 0; i < len(copy.Content); i += 2 {
			name, ok := yamlStringKey(copy.Content[i])
			fieldType, exists := known[name]
			if ok && exists {
				for fieldType.Kind() == reflect.Pointer {
					fieldType = fieldType.Elem()
				}
				value := copy.Content[i+1]
				for value.Kind == yaml.AliasNode {
					value = value.Alias
				}
				// go-yaml coerces YAML 1.1 strings such as "yes" into bools;
				// serde-compatible records require an actual boolean scalar.
				if fieldType.Kind() == reflect.Bool && value.Tag != "!!bool" && value.Tag != "!!null" {
					return fmt.Errorf("field %q must be a boolean", name)
				}
				content = append(content, copy.Content[i], copy.Content[i+1])
			}
		}
		copy.Content = content
	}
	return copy.Decode(target)
}
