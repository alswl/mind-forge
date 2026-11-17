package model

type Reference struct {
	FromKind string  `json:"from_kind"`
	FromID   string  `json:"from_id"`
	FromPath *string `json:"from_path"`
	Line     *uint64 `json:"line"`
	Kind     string  `json:"kind"`
}

type PlannedChange struct {
	Op   string  `json:"op"`
	Path string  `json:"path"`
	Old  *string `json:"old"`
	New  *string `json:"new"`
}

type ScopeRef struct {
	Project *string `json:"project"`
	Global  bool    `json:"global"`
}
