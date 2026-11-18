package model

import (
	"encoding/json"
	"fmt"
	"strings"

	"go.yaml.in/yaml/v3"
)

type ExtraField struct {
	Key   string
	Value any
}

func (field ExtraField) MarshalJSON() ([]byte, error) {
	return json.Marshal([2]any{field.Key, field.Value})
}

type Index struct {
	SchemaVersion  string           `json:"schema_version" yaml:"schema_version"`
	Sources        *[]Source        `json:"sources,omitempty" yaml:"sources,omitempty"`
	Assets         *[]Asset         `json:"assets,omitempty" yaml:"assets,omitempty"`
	Articles       *[]Article       `json:"articles,omitempty" yaml:"articles,omitempty"`
	Prompts        *[]Prompt        `json:"prompts,omitempty" yaml:"prompts,omitempty"`
	Thinking       *[]Thinking      `json:"thinking,omitempty" yaml:"thinking,omitempty"`
	Terms          *[]Term          `json:"terms,omitempty" yaml:"terms,omitempty"`
	PublishRecords *[]PublishRecord `json:"publish_records,omitempty" yaml:"publish_records,omitempty"`
	Extra          []ExtraField     `json:"extra,omitempty" yaml:"extra,omitempty"`
}

func DefaultIndex() Index {
	return Index{SchemaVersion: SchemaVersion}
}

func (index *Index) UnmarshalYAML(node *yaml.Node) error {
	if _, err := mapping(node); err != nil {
		return err
	}
	result := DefaultIndex()
	sections := make(map[string]*yaml.Node)
	for position := 0; position < len(node.Content); position += 2 {
		key, _ := yamlStringKey(node.Content[position])
		value := node.Content[position+1]
		switch key {
		case "schema_version", "schema":
			result.SchemaVersion = SchemaVersion
			if scalar, ok := yamlStringKey(value); ok {
				result.SchemaVersion = scalar
			}
		case "sources", "files":
			incoming := normalizeSection(value, key)
			if previous := sections["sources"]; previous != nil && previous.Kind == yaml.SequenceNode && incoming.Kind == yaml.SequenceNode {
				previous.Content = append(previous.Content, incoming.Content...)
			} else {
				sections["sources"] = incoming
			}
		case "assets", "articles", "prompts", "thinking":
			sections[key] = normalizeSection(value, key)
		case "terms", "publish_records":
			sections[key] = value
		default:
			result.Extra = append(result.Extra, ExtraField{Key: key, Value: yamlJSONValue{node: value, mode: indexNormalize}})
		}
	}
	var err error
	if result.Sources, err = decodeSection[Source](sections["sources"], "sources"); err != nil {
		return err
	}
	if result.Assets, err = decodeSection[Asset](sections["assets"], "assets"); err != nil {
		return err
	}
	if result.Articles, err = decodeSection[Article](sections["articles"], "articles"); err != nil {
		return err
	}
	if result.Prompts, err = decodeSection[Prompt](sections["prompts"], "prompts"); err != nil {
		return err
	}
	if result.Thinking, err = decodeSection[Thinking](sections["thinking"], "thinking"); err != nil {
		return err
	}
	if result.Terms, err = decodeSection[Term](sections["terms"], "terms"); err != nil {
		return err
	}
	if result.PublishRecords, err = decodeSection[PublishRecord](sections["publish_records"], "publish_records"); err != nil {
		return err
	}
	*index = result
	return nil
}

func decodeSection[T any](node *yaml.Node, name string) (*[]T, error) {
	for node != nil && node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	if node == nil || node.Kind != yaml.SequenceNode {
		return nil, nil
	}
	result := make([]T, 0, len(node.Content))
	for position, item := range node.Content {
		resolved := item
		for resolved.Kind == yaml.AliasNode {
			resolved = resolved.Alias
		}
		if resolved.Tag == "!!null" {
			return nil, fmt.Errorf("invalid %s[%d]: null record", name, position)
		}
		var value T
		if err := item.Decode(&value); err != nil {
			return nil, fmt.Errorf("invalid %s[%d]: %w", name, position, err)
		}
		result = append(result, value)
	}
	return &result, nil
}

func normalizeSection(node *yaml.Node, name string) *yaml.Node {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	if node.Tag == "!!null" {
		return &yaml.Node{Kind: yaml.SequenceNode, Tag: "!!seq"}
	}
	if node.Kind != yaml.MappingNode {
		// Own the sequence slice because merging files and sources must not
		// mutate the raw document's AST, even though its entry nodes are shared.
		copy := *node
		copy.Content = append([]*yaml.Node(nil), node.Content...)
		return &copy
	}
	sequence := &yaml.Node{Kind: yaml.SequenceNode, Tag: "!!seq"}
	for position := 0; position < len(node.Content); position += 2 {
		keyNode, value := node.Content[position], node.Content[position+1]
		for value.Kind == yaml.AliasNode {
			value = value.Alias
		}
		key, _ := yamlStringKey(keyNode)
		entry := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		if value.Kind == yaml.MappingNode {
			entry.Content = append(entry.Content, value.Content...)
		}
		field := func(key string) *yaml.Node {
			for i := 0; i < len(entry.Content); i += 2 {
				if entry.Content[i].Value == key {
					return entry.Content[i+1]
				}
			}
			return nil
		}
		setDefault := func(key string, value *yaml.Node) {
			if field(key) == nil {
				entry.Content = append(entry.Content, scalarString(key), value)
			}
		}
		switch name {
		case "sources", "files":
			setDefault("name", scalarString(key))
			if name == "files" || strings.Contains(key, "/") || strings.HasPrefix(key, ".") {
				setDefault("path", keyNode)
			}
			if kind := field("type"); name == "files" && kind != nil && kind.Value == "markdown" {
				for i := 0; i < len(entry.Content); i += 2 {
					if entry.Content[i].Value == "type" {
						entry.Content[i+1] = scalarString("file")
					}
				}
			}
			setDefault("added_at", scalarString(""))
			setDefault("updated_at", scalarString(""))
		case "assets":
			setDefault("name", scalarString(key))
			setDefault("added_at", scalarString(""))
			setDefault("hash", scalarString(""))
			setDefault("size", &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!int", Value: "0"})
		case "articles":
			title := key
			if value.Kind == yaml.MappingNode {
				title = strings.ReplaceAll(key, "-", " ")
			}
			setDefault("title", scalarString(title))
			setDefault("project", scalarString(""))
			path := key
			if !strings.HasPrefix(path, "docs/") {
				path = "docs/" + path
			}
			setDefault("article_path", scalarString(path))
			setDefault("created_at", scalarString(""))
			setDefault("updated_at", scalarString(""))
			setDefault("type", scalarString("blog"))
			setDefault("status", scalarString("draft"))
		case "prompts", "thinking":
			setDefault("path", scalarString(name+"/"+key+".md"))
			setDefault("article", scalarString(""))
			setDefault("updated_at", scalarString(""))
		}
		sequence.Content = append(sequence.Content, entry)
	}
	return sequence
}

type Prompt struct {
	Path      string  `json:"path" yaml:"path"`
	Article   string  `json:"article" yaml:"article"`
	Mode      *string `json:"mode,omitempty" yaml:"mode,omitempty"`
	UpdatedAt string  `json:"updated_at" yaml:"updated_at"`
}

type Thinking struct {
	Path      string `json:"path" yaml:"path"`
	Article   string `json:"article" yaml:"article"`
	UpdatedAt string `json:"updated_at" yaml:"updated_at"`
}

type PublishRecord struct {
	Path        string  `json:"path" yaml:"path"`
	TargetName  string  `json:"target_name" yaml:"target_name"`
	Status      string  `json:"status" yaml:"status"`
	TargetURL   *string `json:"target_url" yaml:"target_url"`
	PublishedAt *string `json:"published_at" yaml:"published_at"`
}

func (prompt *Prompt) UnmarshalYAML(node *yaml.Node) error {
	type wire Prompt
	var result wire
	if _, err := decodeRecord(node, &result, nil, "path", "article", "updated_at"); err != nil {
		return err
	}
	if result.Mode != nil {
		if err := enumValue(*result.Mode, "editorial", "research", "decision-research"); err != nil {
			return err
		}
	}
	*prompt = Prompt(result)
	return nil
}

func (thinking *Thinking) UnmarshalYAML(node *yaml.Node) error {
	type wire Thinking
	var result wire
	if _, err := decodeRecord(node, &result, nil, "path", "article", "updated_at"); err != nil {
		return err
	}
	*thinking = Thinking(result)
	return nil
}

func (record *PublishRecord) UnmarshalYAML(node *yaml.Node) error {
	type wire PublishRecord
	var result wire
	if _, err := decodeRecord(node, &result, []string{"path", "target_name", "status"}); err != nil {
		return err
	}
	if err := enumValue(result.Status, "draft", "published", "archived"); err != nil {
		return err
	}
	*record = PublishRecord(result)
	return nil
}
