package model

type ScannedArticle struct {
	Title       string  `json:"title"`
	Filename    string  `json:"filename"`
	ArticleDir  *string `json:"article_dir,omitempty"`
	ArticlePath *string `json:"article_path,omitempty"`
}

type ArticleDiff struct {
	Added   JSONArray[ScannedArticle] `json:"added"`
	Removed JSONArray[Article]        `json:"removed"`
}

type ArticleRemoveReport struct {
	Verb        string                   `json:"verb"`
	Kind        string                   `json:"kind"`
	Before      ArticleIdentity          `json:"before"`
	After       *ArticleIdentity         `json:"after,omitempty"`
	References  JSONArray[Reference]     `json:"references"`
	SideEffects JSONArray[PlannedChange] `json:"side_effects"`
	Force       bool                     `json:"force"`
	DryRun      bool                     `json:"dry_run"`
}

type ArticleIdentity struct {
	Title       string   `json:"title"`
	ArticlePath string   `json:"article_path"`
	Scope       ScopeRef `json:"scope"`
}

type LintIssue struct {
	Severity string `json:"severity"`
	Kind     string `json:"kind"`
	Message  string `json:"message"`
	Path     string `json:"path"`
	Fixable  bool   `json:"fixable"`
}

type ConversionResult struct {
	Status             string             `json:"status"`
	Direction          string             `json:"direction"`
	SourceShape        string             `json:"source_shape"`
	TargetShape        string             `json:"target_shape"`
	SourcePath         string             `json:"source_path"`
	SourceContentPath  string             `json:"source_content_path"`
	TargetPath         string             `json:"target_path"`
	TargetContentPath  string             `json:"target_content_path"`
	Reason             *string            `json:"reason,omitempty"`
	IndexUpdated       bool               `json:"index_updated"`
	SourceRemoved      bool               `json:"source_removed"`
	MergedSectionFiles *JSONArray[string] `json:"merged_section_files,omitempty"`
}

type ConversionSummary struct {
	Kind            string                      `json:"kind"`
	Direction       string                      `json:"direction"`
	DirectionSource string                      `json:"direction_source"`
	DryRun          bool                        `json:"dry_run"`
	ConvertedCount  uint64                      `json:"converted_count"`
	SkippedCount    uint64                      `json:"skipped_count"`
	FailedCount     uint64                      `json:"failed_count"`
	ScannedCount    uint64                      `json:"scanned_count"`
	Converted       JSONArray[ConversionResult] `json:"converted"`
	Skipped         JSONArray[ConversionResult] `json:"skipped"`
	Failed          JSONArray[ConversionResult] `json:"failed"`
}
