package model

type AssetUpdateResult struct {
	Path    string  `json:"path"`
	Changed bool    `json:"changed"`
	OldSize uint64  `json:"old_size"`
	NewSize uint64  `json:"new_size"`
	OldHash string  `json:"old_hash"`
	NewHash string  `json:"new_hash"`
	Error   *string `json:"error,omitempty"`
}

type AssetIndexEntry struct {
	Name string `json:"name"`
	Path string `json:"path"`
}

type AssetIndexReport struct {
	Added     JSONArray[AssetIndexEntry]    `json:"added"`
	Removed   JSONArray[AssetIndexEntry]    `json:"removed"`
	KeptCount uint64                        `json:"kept_count"`
	Refreshed *JSONArray[AssetUpdateResult] `json:"refreshed,omitempty"`
}

type AssetCleanReport struct {
	StaleEntries JSONArray[string] `json:"stale_entries"`
	RemovedCount uint64            `json:"removed_count"`
	DryRun       bool              `json:"dry_run"`
}

type AssetRemoveReport struct {
	Removed       string                   `json:"removed"`
	WasReferenced bool                     `json:"was_referenced"`
	References    JSONArray[Reference]     `json:"references,omitempty"`
	SideEffects   JSONArray[PlannedChange] `json:"side_effects,omitempty"`
	Force         bool                     `json:"force"`
	DryRun        bool                     `json:"dry_run"`
}
