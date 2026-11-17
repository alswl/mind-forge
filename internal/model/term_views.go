package model

type TermFinding struct {
	Path                  string                   `json:"path"`
	Line                  uint32                   `json:"line"`
	Column                uint32                   `json:"column"`
	Original              string                   `json:"original"`
	Correct               string                   `json:"correct"`
	Term                  string                   `json:"term"`
	Description           *string                  `json:"description,omitempty"`
	Confidence            *float64                 `json:"confidence,omitempty"`
	ReplacementEligible   bool                     `json:"replacement_eligible"`
	SafetyReason          *string                  `json:"safety_reason,omitempty"`
	Candidates            JSONArray[CandidateTerm] `json:"candidates,omitempty"`
	MatchKind             string                   `json:"match_kind"`
	FixKind               string                   `json:"fix_kind"`
	Boundary              string                   `json:"boundary"`
	BoundaryMode          string                   `json:"boundary_mode"`
	SubstringAdjacentWord bool                     `json:"substring_adjacent_word,omitempty"`
	QuoteProtected        bool                     `json:"quote_protected,omitempty"`
	Selection             string                   `json:"selection"`
	Context               string                   `json:"context"`
	HeldBack              bool                     `json:"held_back"`
	CompetingTerms        JSONArray[string]        `json:"competing_terms,omitempty"`
}

type CandidateTerm struct {
	Term       string   `json:"term"`
	Correct    string   `json:"correct"`
	Confidence *float64 `json:"confidence,omitempty"`
}

type TermLintFailure struct {
	Path   string `json:"path"`
	Reason string `json:"reason"`
}

type CorrectionSelector struct {
	Original string `json:"original"`
}

type CorrectionPatch struct {
	Correct  *string  `json:"correct,omitempty"`
	Match    *string  `json:"match,omitempty"`
	Fix      *string  `json:"fix,omitempty"`
	Pinyin   **string `json:"pinyin,omitempty"`
	Boundary *string  `json:"boundary,omitempty"`
}

type CorrectionChangeReport struct {
	Term       string            `json:"term"`
	Scope      string            `json:"scope"`
	Correction Correction        `json:"correction"`
	DryRun     bool              `json:"dry_run"`
	Changes    JSONArray[string] `json:"changes,omitempty"`
}

type MoveSideEffect struct {
	Action      string `json:"action"`
	Scope       string `json:"scope"`
	Description string `json:"description"`
}

type TermMoveReport struct {
	Term        string                    `json:"term"`
	FromScope   string                    `json:"from_scope"`
	ToScope     string                    `json:"to_scope"`
	DryRun      bool                      `json:"dry_run"`
	Force       bool                      `json:"force"`
	SideEffects JSONArray[MoveSideEffect] `json:"side_effects,omitempty"`
}

type TermCheckTarget struct {
	TargetType string  `json:"target_type"`
	Project    *string `json:"project,omitempty"`
	Article    *string `json:"article,omitempty"`
	Path       *string `json:"path,omitempty"`
}

type TermRemoveReport struct {
	Verb        string                   `json:"verb"`
	Kind        string                   `json:"kind"`
	Before      TermIdentity             `json:"before"`
	After       *TermIdentity            `json:"after,omitempty"`
	References  JSONArray[Reference]     `json:"references"`
	SideEffects JSONArray[PlannedChange] `json:"side_effects"`
	Force       bool                     `json:"force"`
	DryRun      bool                     `json:"dry_run"`
}

type TermIdentity struct {
	Name  string   `json:"name"`
	Scope ScopeRef `json:"scope"`
}

type TermLintReport struct {
	NoTermsRegistered    bool                       `json:"-"`
	Findings             JSONArray[TermFinding]     `json:"findings"`
	ScannedFiles         uint64                     `json:"scanned_files"`
	SkippedFiles         JSONArray[string]          `json:"skipped_files"`
	FixedCount           uint64                     `json:"fixed_count"`
	ModifiedFiles        JSONArray[string]          `json:"modified_files"`
	Failures             JSONArray[TermLintFailure] `json:"failures"`
	WouldFixCount        *uint64                    `json:"would_fix_count,omitempty"`
	WouldApplyCount      uint64                     `json:"would_apply_count"`
	SelectedCount        uint64                     `json:"selected_count"`
	ExcludedCount        uint64                     `json:"excluded_count"`
	BelowConfidenceCount uint64                     `json:"below_confidence_count"`
	IneligibleCount      uint64                     `json:"ineligible_count"`
	HeldBackCount        uint64                     `json:"held_back_count"`
	QuoteProtectedCount  uint64                     `json:"quote_protected_count,omitempty"`
}
