package model

type PromptIndexReport struct {
	Added     JSONArray[Prompt] `json:"added"`
	Removed   JSONArray[Prompt] `json:"removed"`
	KeptCount uint64            `json:"kept_count"`
	DryRun    bool              `json:"dry_run"`
}
