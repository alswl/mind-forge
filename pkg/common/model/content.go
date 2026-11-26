package model

import (
	"fmt"
	"slices"

	"go.yaml.in/yaml/v3"
)

type MindRegistration struct {
	Title         *string `json:"title" yaml:"title"`
	DefaultOutput *string `json:"default_output" yaml:"default_output"`
}

func (registration *MindRegistration) UnmarshalYAML(node *yaml.Node) error {
	type wire MindRegistration
	var result wire
	if _, err := decodeRecord(node, &result, nil); err != nil {
		return err
	}
	*registration = MindRegistration(result)
	return nil
}

type Mind struct {
	Identity        string             `json:"identity"`
	ProjectIdentity string             `json:"project_identity"`
	MindIdentity    string             `json:"mind_identity"`
	Name            string             `json:"name"`
	Title           string             `json:"title"`
	Path            *string            `json:"path"`
	DefaultOutput   *string            `json:"default_output"`
	Roles           JSONObject[string] `json:"roles"`
	Outputs         JSONArray[Output]  `json:"outputs"`
}

type Output struct {
	Identity        string `json:"identity"`
	ProjectIdentity string `json:"project_identity"`
	MindIdentity    string `json:"mind_identity"`
	OutputIdentity  string `json:"output_identity"`
	Mind            string `json:"mind"`
	Name            string `json:"name"`
	Path            string `json:"path"`
	Shape           string `json:"shape"`
	BuildPath       string `json:"build_path"`
}

type Source struct {
	Name       string            `json:"name" yaml:"name"`
	Kind       string            `json:"type" yaml:"type"`
	SourceKind *string           `json:"source_kind,omitempty" yaml:"source_kind,omitempty"`
	URL        *string           `json:"url" yaml:"url"`
	Path       *string           `json:"path" yaml:"path"`
	Tags       JSONArray[string] `json:"tags" yaml:"tags"`
	AddedAt    string            `json:"added_at" yaml:"added_at"`
	UpdatedAt  string            `json:"updated_at" yaml:"updated_at"`
	Extra      YAMLFields        `json:"-" yaml:"-"`
}

func (source *Source) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "name"); err != nil {
		return err
	}
	if err := rejectNull(fields, "type", "file_kind", "tags", "added_at", "updated_at"); err != nil {
		return err
	}
	if err := rejectNullItems(fields["tags"]); err != nil {
		return err
	}
	if fields["type"] != nil && fields["file_kind"] != nil {
		return fmt.Errorf("duplicate field %q", "type")
	}
	type wire Source
	result := wire{Kind: "file", Tags: []string{}}
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	if alias := fields["file_kind"]; alias != nil {
		if err := alias.Decode(&result.Kind); err != nil {
			return err
		}
	}
	if err := enumValue(result.Kind, "auto", "pdf", "rss", "web", "file"); err != nil {
		return err
	}
	result.Extra = unknownFields(node, "name", "type", "file_kind", "source_kind", "url", "path", "tags", "added_at", "updated_at")
	if err := result.Extra.validateFlattened(); err != nil {
		return err
	}
	*source = Source(result)
	return nil
}

func (source Source) MarshalJSON() ([]byte, error) {
	fields := map[string]any{
		"name": source.Name, "type": source.Kind, "url": source.URL, "path": source.Path,
		"tags": source.Tags, "added_at": source.AddedAt, "updated_at": source.UpdatedAt,
	}
	if source.SourceKind != nil {
		fields["source_kind"] = source.SourceKind
	}
	return flattenJSON(fields, source.Extra)
}

func (source Source) MarshalYAML() (any, error) {
	type wire Source
	return flattenYAML(wire(source), source.Extra)
}

type Asset struct {
	Name    string            `json:"name" yaml:"name"`
	Kind    string            `json:"type" yaml:"type"`
	Path    string            `json:"path" yaml:"path"`
	Size    uint64            `json:"size" yaml:"size"`
	Hash    string            `json:"hash" yaml:"hash"`
	Tags    JSONArray[string] `json:"tags" yaml:"tags"`
	AddedAt string            `json:"added_at" yaml:"added_at"`
}

func (asset *Asset) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "name"); err != nil {
		return err
	}
	if err := rejectNull(fields, "type", "path", "size", "hash", "tags", "added_at"); err != nil {
		return err
	}
	if err := rejectNullItems(fields["tags"]); err != nil {
		return err
	}
	type wire Asset
	result := wire{Kind: "other", Tags: []string{}}
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	if err := enumValue(result.Kind, "image", "video", "audio", "other"); err != nil {
		return err
	}
	*asset = Asset(result)
	return nil
}

type TemplateOrigin struct {
	TemplateName string `json:"template_name" yaml:"template_name"`
	SlotValue    string `json:"slot_value" yaml:"slot_value"`
}

func (origin *TemplateOrigin) UnmarshalYAML(node *yaml.Node) error {
	type wire TemplateOrigin
	var result wire
	if _, err := decodeRecord(node, &result, []string{"template_name", "slot_value"}); err != nil {
		return err
	}
	*origin = TemplateOrigin(result)
	return nil
}

type Article struct {
	Title          string          `json:"title" yaml:"title"`
	Project        string          `json:"project" yaml:"project"`
	Type           string          `json:"type" yaml:"type"`
	Path           string          `json:"article_path" yaml:"article_path"`
	Status         string          `json:"status" yaml:"status"`
	CreatedAt      string          `json:"created_at" yaml:"created_at"`
	UpdatedAt      string          `json:"updated_at" yaml:"updated_at"`
	TemplateOrigin *TemplateOrigin `json:"template_origin,omitempty" yaml:"template_origin,omitempty"`
}

const (
	ArticleStatusDraft     = "draft"
	ArticleStatusPublished = "published"
	ArticleStatusCompleted = "completed"
	ArticleStatusAbandoned = "abandoned"
)

func ValidateArticleStatus(status string) error {
	return enumValue(status, ArticleStatusDraft, ArticleStatusPublished, ArticleStatusCompleted, ArticleStatusAbandoned)
}

func (article *Article) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "title"); err != nil {
		return err
	}
	if err := rejectNull(fields, "project", "type", "article_path", "status", "created_at", "updated_at"); err != nil {
		return err
	}
	type wire Article
	result := wire{Type: "blank", Status: ArticleStatusDraft}
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	if err := enumValue(result.Type, "arch", "prd", "blog", "blank"); err != nil {
		return err
	}
	if err := ValidateArticleStatus(result.Status); err != nil {
		return err
	}
	*article = Article(result)
	return nil
}

func enumValue(value string, choices ...string) error {
	if !slices.Contains(choices, value) {
		return fmt.Errorf("unknown variant %q, expected one of %v", value, choices)
	}
	return nil
}
