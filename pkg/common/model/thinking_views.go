package model

type ThinkingIndexReport struct {
	Added     JSONArray[Thinking] `json:"added"`
	Removed   JSONArray[Thinking] `json:"removed"`
	KeptCount uint64              `json:"kept_count"`
	DryRun    bool                `json:"dry_run"`
}
