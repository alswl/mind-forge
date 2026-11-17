package model

type SourceIndexEntry struct {
	Name string `json:"name"`
	Kind string `json:"type"`
	Path string `json:"path"`
}

type SourceIndexReport struct {
	Added     JSONArray[SourceIndexEntry] `json:"added"`
	Removed   JSONArray[SourceIndexEntry] `json:"removed"`
	KeptCount uint64                      `json:"kept_count"`
	DryRun    bool                        `json:"dry_run"`
}

type SourceRemoveReport struct {
	Source      Source                   `json:"-"`
	FileDeleted bool                     `json:"file_deleted"`
	References  JSONArray[Reference]     `json:"references,omitempty"`
	SideEffects JSONArray[PlannedChange] `json:"side_effects,omitempty"`
	Force       bool                     `json:"force"`
	DryRun      bool                     `json:"dry_run"`
}
