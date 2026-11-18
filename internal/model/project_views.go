package model

type ProjectListEntry struct {
	Name           string  `json:"name"`
	Path           string  `json:"path"`
	CreatedAt      string  `json:"created_at"`
	ArchivedAt     *string `json:"archived_at"`
	DocumentCount  uint64  `json:"document_count"`
	LastActivityAt *string `json:"last_activity_at"`
}

type ProjectLintIssue struct {
	Severity string `json:"severity"`
	Kind     string `json:"kind"`
	Message  string `json:"message"`
	Path     string `json:"path"`
	Fixable  bool   `json:"fixable"`
	Fixed    bool   `json:"fixed"`
}

type MindYamlSummary struct {
	SchemaVersion string            `json:"schema_version"`
	Types         JSONArray[string] `json:"types"`
	SourceDirs    JSONArray[string] `json:"source_dirs"`
	AssetsDir     string            `json:"assets_dir"`
}

type ProjectDetails struct {
	Name            string           `json:"name"`
	Path            string           `json:"path"`
	ArticleCount    uint64           `json:"article_count"`
	SourceCount     uint64           `json:"source_count"`
	AssetCount      uint64           `json:"asset_count"`
	LastActive      *string          `json:"last_active"`
	MindYamlSummary *MindYamlSummary `json:"mind_yaml_summary"`
	Layout          *EffectiveLayout `json:"layout"`
}

type ProjectArchiveReport struct {
	Name string `json:"name"`
	From string `json:"from"`
	To   string `json:"to"`
}

type ProjectImportReport struct {
	Name         string `json:"name"`
	Path         string `json:"path"`
	Scaffolded   bool   `json:"scaffolded"`
	ArticleCount uint64 `json:"article_count"`
}

type ProjectRemoveReport struct {
	Verb        string                   `json:"verb"`
	Kind        string                   `json:"kind"`
	Before      ProjectIdentity          `json:"before"`
	After       *ProjectIdentity         `json:"after,omitempty"`
	References  JSONArray[Reference]     `json:"references"`
	SideEffects JSONArray[PlannedChange] `json:"side_effects"`
	Force       bool                     `json:"force"`
	DryRun      bool                     `json:"dry_run"`
}

type ProjectIdentity struct {
	Name string `json:"name"`
	Path string `json:"path"`
}
