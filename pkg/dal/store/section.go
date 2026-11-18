package store

import (
	"bytes"
	"sort"

	"github.com/alswl/mind-forge/pkg/common/model"
	"go.yaml.in/yaml/v3"
)

func replaceProtectedSection(raw []byte, section string, rendered []byte) ([]byte, error) {
	document, err := ParseYAML(raw)
	if err != nil {
		return nil, err
	}
	if len(document.Root.Content) == 0 {
		return replaceIndexSection(raw, section, rendered), nil
	}
	root := document.Root.Content[0]
	if root.Kind != yaml.MappingNode {
		return replaceIndexSection(raw, section, rendered), nil
	}
	anchors := map[string]bool{}
	var collect func(*yaml.Node)
	collect = func(node *yaml.Node) {
		if node == nil {
			return
		}
		if node.Anchor != "" {
			anchors[node.Anchor] = true
		}
		for _, child := range node.Content {
			collect(child)
		}
	}
	var refers func(*yaml.Node) bool
	refers = func(node *yaml.Node) bool {
		if node.Kind == yaml.AliasNode && anchors[node.Value] {
			return true
		}
		for _, child := range node.Content {
			if refers(child) {
				return true
			}
		}
		return false
	}
	for i := 0; i < len(root.Content); i += 2 {
		if root.Content[i].Value == section {
			collect(root.Content[i+1])
		}
	}
	updates := map[string]*yaml.Node{}
	for changed := true; changed; {
		changed = false
		for i := 0; i < len(root.Content); i += 2 {
			name, node := root.Content[i].Value, root.Content[i+1]
			if name == section || updates[name] != nil || !refers(node) {
				continue
			}
			updates[name], err = model.CloneYAMLValue(node)
			if err != nil {
				return nil, err
			}
			collect(node)
			changed = true
		}
	}
	blocks := map[string][]byte{section: rendered}
	names := []string{section}
	for name, node := range updates {
		block, err := yaml.Marshal(map[string]any{name: node})
		if err != nil {
			return nil, err
		}
		blocks[name] = block
		names = append(names, name)
	}
	fallback := root.Style&yaml.FlowStyle != 0
	for i := 0; i < len(root.Content); i += 2 {
		if _, changed := blocks[root.Content[i].Value]; changed {
			if _, _, span := topLevelSpan(raw, root.Content[i].Value); !span {
				fallback = true
			}
		}
	}
	crlf := bytes.Contains(raw, []byte("\r\n"))
	if fallback {
		expanded, err := model.CloneYAMLValue(root)
		if err != nil {
			return nil, err
		}
		replacements := map[string]*yaml.Node{}
		for name, block := range blocks {
			if len(block) == 0 {
				replacements[name] = nil
				continue
			}
			doc, err := ParseYAML(block)
			if err != nil {
				return nil, err
			}
			replacements[name] = doc.Root.Content[0].Content[1]
		}
		content := []*yaml.Node{}
		seen := map[string]bool{}
		for i := 0; i < len(expanded.Content); i += 2 {
			key, value := expanded.Content[i], expanded.Content[i+1]
			if replacement, changed := replacements[key.Value]; changed {
				if replacement == nil || seen[key.Value] {
					continue
				}
				value = replacement
				seen[key.Value] = true
			}
			content = append(content, key, value)
		}
		for _, name := range names {
			if !seen[name] && replacements[name] != nil {
				content = append(content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: name}, replacements[name])
			}
		}
		expanded.Content = content
		result, err := yaml.Marshal(expanded)
		if crlf {
			result = bytes.ReplaceAll(result, []byte("\n"), []byte("\r\n"))
		}
		return result, err
	}
	sort.Strings(names)
	for _, name := range names {
		block := blocks[name]
		if crlf && !bytes.Contains(block, []byte("\r\n")) {
			block = bytes.ReplaceAll(block, []byte("\n"), []byte("\r\n"))
		}
		raw = replaceIndexSection(raw, name, block)
	}
	return raw, nil
}
