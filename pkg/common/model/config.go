package model

import (
	"fmt"

	"go.yaml.in/yaml/v3"
)

type ProjectMeta struct {
	Name        string  `json:"name" yaml:"name"`
	Description *string `json:"description,omitempty" yaml:"description,omitempty"`
	CreatedAt   *string `json:"created_at" yaml:"created_at"`
}

func (meta *ProjectMeta) UnmarshalYAML(node *yaml.Node) error {
	type wire ProjectMeta
	var result wire
	if _, err := decodeRecord(node, &result, []string{"name"}); err != nil {
		return err
	}
	*meta = ProjectMeta(result)
	return nil
}

type PublishTarget struct {
	Name      string  `json:"name" yaml:"name"`
	Type      string  `json:"type" yaml:"type"`
	Enabled   bool    `json:"enabled" yaml:"enabled"`
	Config    any     `json:"config,omitempty" yaml:"-"`
	Path      *string `json:"path,omitempty" yaml:"path,omitempty"`
	Prefix    *string `json:"prefix,omitempty" yaml:"prefix,omitempty"`
	BookSlug  *string `json:"book_slug,omitempty" yaml:"book_slug,omitempty"`
	Namespace *string `json:"namespace,omitempty" yaml:"namespace,omitempty"`
}

func (target *PublishTarget) UnmarshalYAML(node *yaml.Node) error {
	type wire PublishTarget
	result := wire{Enabled: true}
	fields, err := decodeRecord(node, &result, []string{"name", "type"}, "enabled")
	if err != nil {
		return err
	}
	if err := enumValue(result.Type, "local", "yuque-prompt", "yuque", "github_pages", "custom", "yuque_cc"); err != nil {
		return err
	}
	if value := fields["config"]; value != nil && value.Tag != "!!null" {
		converted, err := convertYAMLJSON(value, jsonDeserialize)
		if err != nil {
			return err
		}
		result.Config = converted
		if converted == nil {
			result.Config = jsonValue{}
		}
	}
	*target = PublishTarget(result)
	return nil
}

func (target PublishTarget) MarshalYAML() (any, error) {
	type wire PublishTarget
	values := YAMLFields{}
	if target.Config != nil {
		var node yaml.Node
		if err := node.Encode(nativeJSON(target.Config)); err != nil {
			return nil, err
		}
		values = append(values, YAMLField{scalarString("config"), &node})
	}
	return flattenYAML(wire(target), values)
}

type PublishConfig struct {
	DefaultTarget    *string          `json:"default_target" yaml:"default_target"`
	Targets          *[]PublishTarget `json:"targets,omitempty" yaml:"targets,omitempty"`
	StripFrontMatter *bool            `json:"strip_front_matter,omitempty" yaml:"strip_front_matter,omitempty"`
}

func (publish *PublishConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire PublishConfig
	var result wire
	fields, err := decodeRecord(node, &result, nil)
	if err != nil {
		return err
	}
	if err := rejectNullItems(fields["targets"]); err != nil {
		return err
	}
	*publish = PublishConfig(result)
	return nil
}

type BannerConfig struct {
	Text  string  `json:"text" yaml:"text"`
	Level *string `json:"level,omitempty" yaml:"level,omitempty"`
}

func (banner *BannerConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire BannerConfig
	var result wire
	if _, err := decodeRecord(node, &result, []string{"text"}); err != nil {
		return err
	}
	if result.Level != nil {
		if err := enumValue(*result.Level, "note", "tip", "warning", "danger"); err != nil {
			return err
		}
	}
	*banner = BannerConfig(result)
	return nil
}

type ArticleBuildConfig struct {
	ArticleDir *string `json:"article_dir,omitempty" yaml:"article_dir,omitempty"`
}

func (config *ArticleBuildConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire ArticleBuildConfig
	var result wire
	if _, err := decodeRecord(node, &result, nil); err != nil {
		return err
	}
	*config = ArticleBuildConfig(result)
	return nil
}

type BuildPipelineRule struct {
	Name            string `json:"name" yaml:"name"`
	InputExtension  string `json:"input_extension" yaml:"input_extension"`
	OutputExtension string `json:"output_extension" yaml:"output_extension"`
	Command         string `json:"command" yaml:"command"`
	Dark            bool   `json:"dark" yaml:"dark"`
}

func (rule *BuildPipelineRule) UnmarshalYAML(node *yaml.Node) error {
	type wire BuildPipelineRule
	var result wire
	if _, err := decodeRecord(node, &result, []string{"name", "input_extension", "output_extension", "command"}, "dark"); err != nil {
		return err
	}
	*rule = BuildPipelineRule(result)
	return nil
}

type BuildConfig struct {
	OutputDir    string                         `json:"output_dir" yaml:"output_dir"`
	MergeOrder   JSONArray[string]              `json:"merge_order" yaml:"merge_order"`
	Format       string                         `json:"format" yaml:"format"`
	HTMLTemplate *string                        `json:"html_template,omitempty" yaml:"html_template,omitempty"`
	Banner       *BannerConfig                  `json:"banner,omitempty" yaml:"banner,omitempty"`
	StripFirstH1 bool                           `json:"strip_first_h1" yaml:"strip_first_h1"`
	Pipeline     JSONArray[BuildPipelineRule]   `json:"pipeline,omitempty" yaml:"pipeline,omitempty"`
	Articles     JSONObject[ArticleBuildConfig] `json:"articles" yaml:"articles"`
}

func DefaultBuildConfig() BuildConfig {
	return BuildConfig{OutputDir: "outputs", MergeOrder: []string{}, Format: "md", Pipeline: []BuildPipelineRule{}, Articles: map[string]ArticleBuildConfig{}}
}

func (build *BuildConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire BuildConfig
	result := wire(DefaultBuildConfig())
	fields, err := decodeRecord(node, &result, nil, "merge_order", "format", "strip_first_h1", "pipeline", "articles")
	if err != nil {
		return err
	}
	if value := fields["output_dir"]; value != nil && value.Tag == "!!null" {
		result.OutputDir = "outputs"
	}
	for _, name := range []string{"merge_order", "pipeline", "articles"} {
		if err := rejectNullItems(fields[name]); err != nil {
			return err
		}
	}
	*build = BuildConfig(result)
	return nil
}

type SourceConfig struct {
	ScanPaths JSONArray[string] `json:"scan_paths" yaml:"scan_paths"`
	Types     JSONArray[string] `json:"types" yaml:"types"`
}

func (source *SourceConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire SourceConfig
	result := wire{ScanPaths: []string{}, Types: []string{}}
	fields, err := decodeRecord(node, &result, nil, "scan_paths", "types")
	if err != nil {
		return err
	}
	for _, name := range []string{"scan_paths", "types"} {
		if err := rejectNullItems(fields[name]); err != nil {
			return err
		}
	}
	*source = SourceConfig(result)
	return nil
}

type HeldBackTermPair struct {
	Original string `json:"original" yaml:"original"`
	Correct  string `json:"correct" yaml:"correct"`
}

func (pair *HeldBackTermPair) UnmarshalYAML(node *yaml.Node) error {
	type wire HeldBackTermPair
	var result wire
	if _, err := decodeRecord(node, &result, []string{"original", "correct"}, "original", "correct"); err != nil {
		return err
	}
	if result.Original == "" || result.Correct == "" {
		return fmt.Errorf("held_back_ignore requires nonempty original and correct")
	}
	*pair = HeldBackTermPair(result)
	return nil
}

type TermConfig struct {
	Enabled        bool               `json:"enabled" yaml:"enabled"`
	CaseSensitive  bool               `json:"case_sensitive" yaml:"case_sensitive"`
	HeldBackIgnore []HeldBackTermPair `json:"held_back_ignore,omitempty" yaml:"held_back_ignore,omitempty"`
}

func (term *TermConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire TermConfig
	result := wire{Enabled: true}
	if _, err := decodeRecord(node, &result, nil, "enabled", "case_sensitive", "held_back_ignore"); err != nil {
		return err
	}
	*term = TermConfig(result)
	return nil
}

type LayoutConfig struct {
	Articles    *string `json:"articles" yaml:"articles"`
	Sources     *string `json:"sources" yaml:"sources"`
	Assets      *string `json:"assets" yaml:"assets"`
	Templates   *string `json:"templates" yaml:"templates"`
	BuildOutput *string `json:"build_output" yaml:"build_output"`
}

func (layout *LayoutConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire LayoutConfig
	var result wire
	fields, err := decodeRecord(node, &result, nil)
	if err != nil {
		return err
	}
	for _, pair := range [][2]string{{"docs", "articles"}, {"output_dir", "build_output"}} {
		if fields[pair[0]] != nil && fields[pair[1]] != nil {
			return fmt.Errorf("duplicate field %q", pair[1])
		}
	}
	if value := fields["docs"]; value != nil {
		if err := value.Decode(&result.Articles); err != nil {
			return err
		}
	}
	if value := fields["output_dir"]; value != nil {
		if err := value.Decode(&result.BuildOutput); err != nil {
			return err
		}
	}
	*layout = LayoutConfig(result)
	return nil
}

type EffectiveLayout struct {
	Articles    string `json:"articles" yaml:"articles"`
	Sources     string `json:"sources" yaml:"sources"`
	Assets      string `json:"assets" yaml:"assets"`
	Templates   string `json:"templates" yaml:"templates"`
	BuildOutput string `json:"build_output" yaml:"build_output"`
}

type PathsConfig struct {
	Docs    string `json:"docs" yaml:"docs"`
	Sources string `json:"sources" yaml:"sources"`
	Assets  string `json:"assets" yaml:"assets"`
	Archive string `json:"archive" yaml:"archive"`
}

func DefaultPathsConfig() PathsConfig {
	return PathsConfig{Docs: "docs", Sources: "sources", Assets: "assets", Archive: "_archived"}
}

func (paths *PathsConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire PathsConfig
	result := wire(DefaultPathsConfig())
	if _, err := decodeRecord(node, &result, nil, "docs", "sources", "assets", "archive"); err != nil {
		return err
	}
	*paths = PathsConfig(result)
	return nil
}

type TyporaConfig struct {
	Enabled *bool      `json:"enabled" yaml:"enabled"`
	Extra   YAMLFields `json:"-" yaml:"-"`
}

func (plugin *TyporaConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire TyporaConfig
	var result wire
	_, err := decodeRecord(node, &result, nil)
	if err != nil {
		return err
	}
	result.Extra = unknownFields(node, "enabled")
	if err := result.Extra.validateFlattened(); err != nil {
		return err
	}
	*plugin = TyporaConfig(result)
	return nil
}

func (plugin TyporaConfig) MarshalJSON() ([]byte, error) {
	return flattenJSON(map[string]any{"enabled": plugin.Enabled}, plugin.Extra)
}

func (plugin TyporaConfig) MarshalYAML() (any, error) {
	type wire TyporaConfig
	return flattenYAML(wire(plugin), plugin.Extra)
}

type PluginsConfig struct {
	Typora *TyporaConfig `json:"typora-front-matter" yaml:"typora-front-matter"`
	Extra  YAMLFields    `json:"-" yaml:"-"`
}

func (plugins *PluginsConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire PluginsConfig
	var result wire
	_, err := decodeRecord(node, &result, nil)
	if err != nil {
		return err
	}
	result.Extra = unknownFields(node, "typora-front-matter")
	if err := result.Extra.validateFlattened(); err != nil {
		return err
	}
	*plugins = PluginsConfig(result)
	return nil
}

func (plugins PluginsConfig) MarshalJSON() ([]byte, error) {
	return flattenJSON(map[string]any{"typora-front-matter": plugins.Typora}, plugins.Extra)
}

func (plugins PluginsConfig) MarshalYAML() (any, error) {
	type wire PluginsConfig
	return flattenYAML(wire(plugins), plugins.Extra)
}

type Template struct {
	Pattern string     `json:"pattern" yaml:"pattern"`
	Mode    string     `json:"mode" yaml:"mode"`
	Extra   YAMLFields `json:"-" yaml:"-"`
}

func (template *Template) UnmarshalYAML(node *yaml.Node) error {
	type wire Template
	var result wire
	_, err := decodeRecord(node, &result, []string{"pattern", "mode"})
	if err != nil {
		return err
	}
	result.Extra = unknownFields(node, "pattern", "mode")
	if err := result.Extra.validateFlattened(); err != nil {
		return err
	}
	*template = Template(result)
	return nil
}

func (template Template) MarshalJSON() ([]byte, error) {
	return flattenJSON(map[string]any{"pattern": template.Pattern, "mode": template.Mode}, template.Extra)
}

func (template Template) MarshalYAML() (any, error) {
	type wire Template
	return flattenYAML(wire(template), template.Extra)
}

type MindConfig struct {
	ReservedRoots JSONArray[string]            `json:"reserved_roots,omitempty" yaml:"reserved_roots,omitempty"`
	ContentLayout *string                      `json:"content_layout,omitempty" yaml:"content_layout,omitempty"`
	Minds         JSONObject[MindRegistration] `json:"minds,omitempty" yaml:"minds,omitempty"`
	SchemaVersion string                       `json:"schema_version" yaml:"schema_version"`
	Project       *ProjectMeta                 `json:"project,omitempty" yaml:"project,omitempty"`
	Build         BuildConfig                  `json:"build" yaml:"build"`
	Publish       PublishConfig                `json:"publish" yaml:"publish"`
	Source        SourceConfig                 `json:"source" yaml:"source"`
	Term          TermConfig                   `json:"term" yaml:"term"`
	Paths         PathsConfig                  `json:"-" yaml:"-"`
	Layout        *LayoutConfig                `json:"layout" yaml:"layout"`
	Name          *string                      `json:"name,omitempty" yaml:"name,omitempty"`
	Description   *string                      `json:"description,omitempty" yaml:"description,omitempty"`
	Created       *string                      `json:"created,omitempty" yaml:"created,omitempty"`
	Updated       *string                      `json:"updated,omitempty" yaml:"updated,omitempty"`
	Articles      any                          `json:"articles,omitempty" yaml:"-"`
	Templates     *map[string]Template         `json:"templates,omitempty" yaml:"templates,omitempty"`
	Plugins       *PluginsConfig               `json:"plugins,omitempty" yaml:"plugins,omitempty"`
}

func readDefaults() MindConfig {
	return MindConfig{SchemaVersion: SchemaVersion, Build: DefaultBuildConfig(), Source: SourceConfig{ScanPaths: []string{}, Types: []string{}}, Term: TermConfig{Enabled: true}, Paths: DefaultPathsConfig(), Minds: map[string]MindRegistration{}, ReservedRoots: []string{}}
}

func DefaultMindConfig() MindConfig {
	config := readDefaults()
	articles, sources, assets, templates, build, enabled := "docs", "sources", "assets", "templates", "outputs", true
	config.Layout = &LayoutConfig{Articles: &articles, Sources: &sources, Assets: &assets, Templates: &templates, BuildOutput: &build}
	config.Plugins = &PluginsConfig{Typora: &TyporaConfig{Enabled: &enabled}}
	return config
}

func (config *MindConfig) UnmarshalYAML(node *yaml.Node) error {
	type wire MindConfig
	result := wire(readDefaults())
	fields, err := decodeRecord(node, &result, nil, "schema", "schema_version", "reserved_roots", "minds", "build", "publish", "source", "term", "paths")
	if err != nil {
		return err
	}
	if fields["schema"] != nil && fields["schema_version"] != nil {
		return fmt.Errorf("duplicate field %q", "schema_version")
	}
	if value := fields["schema"]; value != nil {
		if err := value.Decode(&result.SchemaVersion); err != nil {
			return err
		}
	}
	if value := fields["paths"]; value != nil {
		if err := value.Decode(&result.Paths); err != nil {
			return err
		}
	}
	if value := fields["articles"]; value != nil && value.Tag != "!!null" {
		result.Articles = yamlValue(value)
	}
	for _, name := range []string{"templates", "minds", "reserved_roots"} {
		if err := rejectNullItems(fields[name]); err != nil {
			return err
		}
	}
	if result.ContentLayout != nil {
		if err := enumValue(*result.ContentLayout, "category", "bundle"); err != nil {
			return err
		}
	}
	*config = MindConfig(result)
	return nil
}

func (config MindConfig) MarshalYAML() (any, error) {
	type wire MindConfig
	extra := YAMLFields{}
	if config.Articles != nil {
		var node yaml.Node
		if err := node.Encode(config.Articles); err != nil {
			return nil, err
		}
		extra = append(extra, YAMLField{scalarString("articles"), &node})
	}
	return flattenYAML(wire(config), extra)
}

func decodeRecord(node *yaml.Node, target any, required []string, nonNull ...string) (map[string]*yaml.Node, error) {
	fields, err := mapping(node)
	if err != nil {
		return nil, err
	}
	if err := requireFields(fields, required...); err != nil {
		return nil, err
	}
	if err := rejectNull(fields, nonNull...); err != nil {
		return nil, err
	}
	if err := decodeYAMLRecord(node, target); err != nil {
		return nil, err
	}
	return fields, nil
}
