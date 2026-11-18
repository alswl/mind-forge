package outcome

type Verb string

const (
	Create Verb = "create"
	Add    Verb = "add"
	Rename Verb = "rename"
	Remove Verb = "remove"
	Update Verb = "update"
	Move   Verb = "move"
	Index  Verb = "index"
	Lint   Verb = "lint"
)

type VerbResult struct {
	Verb           Verb
	Kind, Identity string
	OldIdentity    *string
	Path           *string
	DryRun         bool
	Details        map[string]any
}
