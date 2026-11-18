package model

type EffectiveDateOut struct {
	Date   string `json:"date"`
	Origin string `json:"origin"`
}

type LocalRunOutcome struct {
	TargetName    string            `json:"target_name"`
	Article       string            `json:"article"`
	Source        string            `json:"source"`
	Destination   string            `json:"destination"`
	SizeBytes     uint64            `json:"size_bytes"`
	DryRun        bool              `json:"dry_run"`
	EffectiveDate *EffectiveDateOut `json:"effective_date,omitempty"`
}

type YuquePromptRunOutcome struct {
	TargetName             string            `json:"target_name"`
	Article                string            `json:"article"`
	ArticlePath            string            `json:"article_path"`
	BuildArtifactPath      string            `json:"build_artifact_path"`
	Content                string            `json:"content"`
	Prompt                 string            `json:"prompt"`
	Envelope               any               `json:"envelope"`
	SuggestedUpdateCommand string            `json:"suggested_update_command"`
	DryRun                 bool              `json:"dry_run"`
	Destination            *string           `json:"destination,omitempty"`
	Transforms             PayloadTransforms `json:"transforms"`
}

type PayloadTransforms struct {
	SVGPNGReplaced JSONArray[string] `json:"svg_png_replaced"`
	SVGPNGMissing  JSONArray[string] `json:"svg_png_missing"`
	BannerInjected bool              `json:"banner_injected"`
}

type PublishUpdateOutcome struct {
	Article    string        `json:"article"`
	TargetName string        `json:"target_name"`
	Action     string        `json:"action"`
	Record     PublishRecord `json:"record"`
	DryRun     bool          `json:"dry_run"`
}
