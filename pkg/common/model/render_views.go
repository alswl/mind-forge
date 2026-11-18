package model

type RenderTemplate struct {
	Name        string  `json:"name"`
	Label       string  `json:"label"`
	Description string  `json:"description"`
	Source      string  `json:"source"`
	Path        *string `json:"path,omitempty"`
	Default     bool    `json:"default"`
}

type GeneratedPrompt struct {
	Prompt         string                `json:"prompt"`
	Template       string                `json:"template"`
	TemplateSource string                `json:"template_source"`
	Scope          string                `json:"scope"`
	Project        string                `json:"project"`
	Article        *string               `json:"article,omitempty"`
	Outputs        JSONArray[OutputInfo] `json:"outputs"`
}

type OutputInfo struct {
	Path      string `json:"path"`
	SizeBytes uint64 `json:"size_bytes"`
}
