package store

import (
	"bytes"

	"go.yaml.in/yaml/v3"
)

type DuplicateKey struct {
	Key  string
	Line int
}

func IndexDuplicates(raw []byte) ([]DuplicateKey, error) {
	document, err := ParseYAML(raw)
	if err != nil {
		return nil, err
	}
	result := []DuplicateKey{}
	visited := map[*yaml.Node]bool{}
	var walk func(*yaml.Node)
	walk = func(node *yaml.Node) {
		if node == nil || visited[node] {
			return
		}
		visited[node] = true
		if node.Kind == yaml.MappingNode {
			seen := map[string]bool{}
			for i := 0; i < len(node.Content); i += 2 {
				key := node.Content[i]
				id := key.Tag + "\x00" + key.Value
				if seen[id] {
					result = append(result, DuplicateKey{Key: key.Value, Line: key.Line})
				}
				seen[id] = true
			}
		}
		for _, child := range node.Content {
			walk(child)
		}
	}
	walk(&document.Root)
	return result, nil
}

func MergeDuplicateIndex(raw []byte) ([]byte, error) {
	document, err := ParseYAML(raw)
	if err != nil {
		return nil, err
	}
	if len(document.Root.Content) != 1 || document.Root.Content[0].Kind != yaml.MappingNode {
		return raw, nil
	}
	root := document.Root.Content[0]
	positions := map[string]int{}
	merged := []*yaml.Node{}
	changed := map[string]bool{}
	for i := 0; i < len(root.Content); i += 2 {
		key := root.Content[i]
		value := root.Content[i+1]
		nested := []string{}
		deduplicateYAML(value, &nested, map[*yaml.Node]bool{})
		if len(nested) > 0 {
			changed[key.Value] = true
		}
		if pos, exists := positions[key.Value]; exists {
			changed[key.Value] = true
			prior := merged[pos+1]
			switch {
			case (key.Value == "terms" || key.Value == "publish_records") && prior.Kind == yaml.SequenceNode && value.Kind == yaml.SequenceNode:
				prior.Content = append(prior.Content, value.Content...)
			case (key.Value == "articles" || key.Value == "sources" || key.Value == "assets") && prior.Kind == yaml.MappingNode && value.Kind == yaml.MappingNode:
				prior.Content = append(prior.Content, value.Content...)
				deduplicateYAML(prior, &nested, map[*yaml.Node]bool{})
			default:
				merged[pos+1] = value
			}
		} else {
			positions[key.Value] = len(merged)
			merged = append(merged, key, value)
		}
	}
	result := bytes.Clone(raw)
	for i := 0; i < len(merged); i += 2 {
		key := merged[i]
		if !changed[key.Value] {
			continue
		}
		mapping := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map", Content: []*yaml.Node{key, merged[i+1]}}
		block, err := yaml.Marshal(mapping)
		if err != nil {
			return nil, err
		}
		if bytes.Contains(raw, []byte("\r\n")) {
			block = bytes.ReplaceAll(block, []byte("\n"), []byte("\r\n"))
		}
		result, err = replaceProtectedSection(result, key.Value, block)
		if err != nil {
			return nil, err
		}
	}
	return result, nil
}
