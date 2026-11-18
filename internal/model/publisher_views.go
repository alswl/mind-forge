package model

type PublishersOutcome struct {
	Publishers  JSONArray[PublisherView]           `json:"publishers"`
	Diagnostics JSONArray[PublisherDiagnosticView] `json:"diagnostics"`
}

type PublisherView struct {
	Name           string            `json:"name"`
	Label          *string           `json:"label,omitempty"`
	Description    *string           `json:"description,omitempty"`
	TargetType     string            `json:"target_type"`
	Enabled        bool              `json:"enabled"`
	SourcePath     string            `json:"source_path"`
	Status         string            `json:"status"`
	RequiredInputs JSONArray[string] `json:"required_inputs"`
}

type PublisherDiagnosticView struct {
	Kind          string  `json:"kind"`
	Path          *string `json:"path"`
	PublisherName *string `json:"publisher_name"`
	Message       string  `json:"message"`
	Hint          *string `json:"hint"`
}
