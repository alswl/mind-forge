package store

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"go.yaml.in/yaml/v3"
)

type IndexDocument struct {
	Value       model.Index
	Warnings    []string
	duplicates  []indexDuplicateKey
	document    *Document
	fingerprint map[string][]byte
}

type indexDuplicateKey struct {
	key      string
	line     int
	topLevel bool
}

func (index *IndexDocument) DiagnosticWarnings() []string {
	warnings := make([]string, 0, len(index.duplicates))
	for _, duplicate := range index.duplicates {
		kind := "duplicate key"
		if duplicate.topLevel {
			kind = "duplicate top-level key"
		}
		warnings = append(warnings, fmt.Sprintf("%s %q at line %d; using last occurrence", kind, duplicate.key, duplicate.line))
	}
	return warnings
}

func ValidateSchema(version, path string) error {
	if version == model.SchemaVersion {
		return nil
	}
	hint := "use a Mind Forge version that supports this schema"
	return &output.Error{Kind: "incompatible_schema", Message: fmt.Sprintf("incompatible schema: found '%s', expected one of [\"1\"]", version), Hint: &hint}
}

func ReadIndex(path string, strict bool) (*IndexDocument, error) {
	raw, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		document, _ := ParseYAML(nil)
		index := &IndexDocument{Value: model.DefaultIndex(), document: document}
		return index, index.capture()
	}
	if err != nil {
		return nil, err
	}
	return DecodeIndex(raw, path, strict)
}

func DecodeIndex(raw []byte, path string, strict bool) (*IndexDocument, error) {
	document, err := ParseYAML(raw)
	if err != nil {
		return nil, output.ParseError(path, err.Error())
	}
	if len(document.Root.Content) == 0 || document.Root.Content[0].Kind != yaml.MappingNode {
		return nil, output.ParseError(path, "expected a YAML mapping for IndexFile")
	}
	index := &IndexDocument{document: document}
	root := cloneYAMLNode(&document.Root, make(map[*yaml.Node]*yaml.Node))
	deduplicateYAML(root, &index.Warnings, make(map[*yaml.Node]bool), &index.duplicates)
	if err := root.Decode(&index.Value); err != nil {
		return nil, output.ParseError(path, err.Error())
	}
	if err := ValidateSchema(index.Value.SchemaVersion, path); err != nil {
		return nil, err
	}
	if strict && index.Value.Terms != nil {
		if err := model.ValidateCorrections(*index.Value.Terms); err != nil {
			return nil, output.Usage(err.Error(), "")
		}
	}
	return index, index.capture()
}

func cloneYAMLNode(node *yaml.Node, copies map[*yaml.Node]*yaml.Node) *yaml.Node {
	if node == nil {
		return nil
	}
	if copy := copies[node]; copy != nil {
		return copy
	}
	copy := *node
	copies[node] = &copy
	copy.Content = make([]*yaml.Node, len(node.Content))
	for index, child := range node.Content {
		copy.Content[index] = cloneYAMLNode(child, copies)
	}
	copy.Alias = cloneYAMLNode(node.Alias, copies)
	return &copy
}

func deduplicateYAML(node *yaml.Node, warnings *[]string, visited map[*yaml.Node]bool, diagnostics ...*[]indexDuplicateKey) {
	var duplicates *[]indexDuplicateKey
	if len(diagnostics) != 0 {
		duplicates = diagnostics[0]
	}
	deduplicateYAMLWithDiagnostics(node, warnings, visited, duplicates, true)
}

func deduplicateYAMLWithDiagnostics(node *yaml.Node, warnings *[]string, visited map[*yaml.Node]bool, duplicates *[]indexDuplicateKey, topLevel bool) {
	if node == nil || visited[node] {
		return
	}
	visited[node] = true
	if node.Kind == yaml.MappingNode {
		last := make(map[string]int)
		for index := 0; index < len(node.Content); index += 2 {
			key := node.Content[index]
			identity := key.Tag + "\x00" + key.Value
			if _, exists := last[identity]; exists {
				*warnings = append(*warnings, fmt.Sprintf("duplicate key %q at line %d; using last occurrence", key.Value, key.Line))
				if duplicates != nil {
					*duplicates = append(*duplicates, indexDuplicateKey{key: key.Value, line: key.Line, topLevel: topLevel})
				}
			}
			last[identity] = index
		}
		content := make([]*yaml.Node, 0, len(node.Content))
		for index := 0; index < len(node.Content); index += 2 {
			key := node.Content[index]
			if last[key.Tag+"\x00"+key.Value] == index {
				content = append(content, key, node.Content[index+1])
			}
		}
		node.Content = content
	}
	for _, child := range node.Content {
		deduplicateYAMLWithDiagnostics(child, warnings, visited, duplicates, topLevel && node.Kind == yaml.DocumentNode)
	}
	deduplicateYAMLWithDiagnostics(node.Alias, warnings, visited, duplicates, false)
}

var indexSectionOrder = []string{"schema", "sources", "assets", "articles", "prompts", "thinking", "terms", "publish_records"}

func indexSections(index model.Index) map[string]any {
	sections := map[string]any{
		"schema": index.SchemaVersion, "sources": index.Sources, "assets": index.Assets,
		"articles": index.Articles, "prompts": index.Prompts, "thinking": index.Thinking,
		"terms": index.Terms, "publish_records": index.PublishRecords,
	}
	for _, extra := range index.Extra {
		if extra.Key != "schema_version" && extra.Key != "extra" {
			sections[extra.Key] = extra.Value
		}
	}
	return sections
}

func (index *IndexDocument) capture() error {
	index.fingerprint = make(map[string][]byte)
	for name, value := range indexSections(index.Value) {
		encoded, err := yaml.Marshal(value)
		if err != nil {
			return err
		}
		index.fingerprint[name] = encoded
	}
	return nil
}

func (index *IndexDocument) Render(updated model.Index) ([]byte, error) {
	if err := ValidateSchema(updated.SchemaVersion, "mind-index.yaml"); err != nil {
		return nil, err
	}
	raw := index.document.Bytes()
	sections := indexSections(updated)
	order := append([]string(nil), indexSectionOrder...)
	for _, field := range updated.Extra {
		if field.Key != "schema_version" && field.Key != "extra" {
			order = append(order, field.Key)
		}
	}
	var removed []string
	for name := range index.fingerprint {
		if _, remains := sections[name]; !remains {
			removed = append(removed, name)
		}
	}
	sort.Strings(removed)
	order = append(order, removed...)
	for _, name := range order {
		value := sections[name]
		fingerprint, err := yaml.Marshal(value)
		if err != nil {
			return nil, err
		}
		if bytes.Equal(fingerprint, index.fingerprint[name]) && !(name == "schema" && len(raw) == 0) {
			continue
		}
		block, err := renderIndexSection(name, value)
		if err != nil {
			return nil, err
		}
		if bytes.Contains(index.document.raw, []byte("\r\n")) {
			block = bytes.ReplaceAll(block, []byte("\n"), []byte("\r\n"))
		}
		raw, err = replaceProtectedSection(raw, name, block)
		if err != nil {
			return nil, err
		}
		if name == "sources" {
			raw = replaceIndexSection(raw, "files", nil)
		}
	}
	return raw, nil
}

func (index *IndexDocument) Write(path string, updated model.Index, dryRun bool) error {
	raw, err := index.Render(updated)
	if err != nil || dryRun {
		return err
	}
	return (AtomicWriter{}).Write(path, raw, 0o644)
}

func replaceIndexSection(raw []byte, name string, rendered []byte) []byte {
	type span struct{ start, end int }
	var spans []span
	for offset := 0; offset < len(raw); {
		start, end, exists := topLevelSpan(raw[offset:], name)
		if !exists {
			break
		}
		spans = append(spans, span{offset + start, offset + end})
		offset += end
	}
	if len(spans) == 0 {
		if len(rendered) == 0 {
			return raw
		}
		return (&Document{raw: raw}).ReplaceSection(name, rendered)
	}
	for index := len(spans) - 1; index >= 0; index-- {
		block := rendered
		if index != len(spans)-1 {
			block = nil
		}
		rangeToReplace := spans[index]
		result := make([]byte, 0, len(raw)+len(block))
		result = append(result, raw[:rangeToReplace.start]...)
		result = append(result, block...)
		raw = append(result, raw[rangeToReplace.end:]...)
	}
	return raw
}

func renderIndexSection(name string, value any) ([]byte, error) {
	if value == nil {
		return nil, nil
	}
	reflected := reflect.ValueOf(value)
	if reflected.Kind() == reflect.Pointer {
		if reflected.IsNil() {
			return nil, nil
		}
		if (name == "sources" || name == "assets" || name == "articles" || name == "prompts" || name == "thinking" || name == "terms" || name == "publish_records") && reflected.Elem().Len() == 0 {
			return nil, nil
		}
	}
	var node yaml.Node
	if err := node.Encode(value); err != nil {
		return nil, err
	}
	if name == "sources" || name == "assets" || name == "articles" || name == "prompts" || name == "thinking" {
		mapping := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		positions := make(map[string]int)
		for index, entry := range node.Content {
			key := ""
			switch name {
			case "sources":
				source := (*value.(*[]model.Source))[index]
				key = source.Name
				if source.Path != nil {
					key = *source.Path
				}
			case "assets":
				key = (*value.(*[]model.Asset))[index].Name
			case "articles":
				key = strings.TrimRight(strings.TrimSuffix((*value.(*[]model.Article))[index].Path, ".md"), "/")
			case "prompts":
				key = projectionKey((*value.(*[]model.Prompt))[index].Path)
			case "thinking":
				key = projectionKey((*value.(*[]model.Thinking))[index].Path)
			}
			if position, exists := positions[key]; exists {
				mapping.Content[position+1] = entry
			} else {
				positions[key] = len(mapping.Content)
				mapping.Content = append(mapping.Content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: key}, entry)
			}
		}
		node = *mapping
	}
	if name == "schema" {
		node.Style = yaml.SingleQuotedStyle
	}
	root := yaml.Node{Kind: yaml.MappingNode, Tag: "!!map", Content: []*yaml.Node{{Kind: yaml.ScalarNode, Tag: "!!str", Value: name}, &node}}
	var buffer bytes.Buffer
	encoder := yaml.NewEncoder(&buffer)
	encoder.SetIndent(2)
	encoder.CompactSeqIndent()
	if err := encoder.Encode(&root); err != nil {
		return nil, err
	}
	if err := encoder.Close(); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func projectionKey(path string) string {
	name := filepath.Base(path)
	if strings.HasPrefix(name, ".") && !strings.Contains(name[1:], ".") {
		return name
	}
	return strings.TrimSuffix(name, filepath.Ext(name))
}
