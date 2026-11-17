package term

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestMergedListFiltersBeforeShadowingAndDisclosesScope(t *testing.T) {
	repo := t.TempDir()
	project := filepath.Join(repo, "project")
	if err := os.Mkdir(project, 0755); err != nil {
		t.Fatal(err)
	}
	global, local := Scope{Root: repo, Kind: "global"}, Scope{Root: project, Kind: "project"}
	if _, err := global.New("API", Input{Aliases: []string{"remote"}}, nil, false); err != nil {
		t.Fatal(err)
	}
	if _, err := local.New("API", Input{Aliases: []string{"local"}}, nil, false); err != nil {
		t.Fatal(err)
	}
	items, err := List(repo, project, ListFilter{})
	if err != nil || len(items) != 1 || items[0].Scope == nil || *items[0].Scope != "project" {
		t.Fatalf("shadow: %+v %v", items, err)
	}
	items, err = List(repo, project, ListFilter{Filter: "remote"})
	if err != nil || len(items) != 1 || *items[0].Scope != "global" {
		t.Fatalf("filter before merge: %+v %v", items, err)
	}
	items, err = List(repo, "", ListFilter{})
	if err != nil || len(items) != 1 || items[0].Scope != nil {
		t.Fatalf("global only must omit scope: %+v %v", items, err)
	}
}

func TestRenameKeepsAliasWithoutRewritingCorrectionAndRemoveChecksReferences(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "project"}
	if _, err := scope.New("API", Input{}, []string{"api:API"}, false); err != nil {
		t.Fatal(err)
	}
	if err := scope.Rename("API", "NewAPI", true, false); err != nil {
		t.Fatal(err)
	}
	tm, err := scope.Show("NewAPI")
	if err != nil || len(tm.Aliases) != 1 || tm.Aliases[0] != "API" || tm.Corrections[0].Correct != "API" {
		t.Fatalf("rename: %+v %v", tm, err)
	}
	raw, err := os.ReadFile(scope.Path())
	if err != nil {
		t.Fatal(err)
	}
	raw = append(raw, []byte("articles:\n- title: Public\n  article_path: docs/public.md\n")...)
	if err := os.WriteFile(scope.Path(), raw, 0644); err != nil {
		t.Fatal(err)
	}
	if err := scope.Remove("NewAPI", false, false); err == nil {
		t.Fatal("referenced removal accepted")
	}
	if err := scope.Remove("NewAPI", true, true); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.Show("NewAPI"); err != nil {
		t.Fatal("dry remove wrote")
	}
	if err := scope.Remove("NewAPI", true, false); err != nil {
		t.Fatal(err)
	}
}

func TestMovePlanRejectsConflictBeforeSourceMutation(t *testing.T) {
	repo := t.TempDir()
	project := filepath.Join(repo, "project")
	if err := os.Mkdir(project, 0755); err != nil {
		t.Fatal(err)
	}
	from, to := Scope{Root: project, Kind: "project"}, Scope{Root: repo, Kind: "global"}
	if _, err := from.New("API", Input{Definition: stringPtr("local")}, nil, false); err != nil {
		t.Fatal(err)
	}
	if _, err := to.New("API", Input{Definition: stringPtr("global")}, nil, false); err != nil {
		t.Fatal(err)
	}
	if _, err := PlanMove(repo, from, to, "API", false); err == nil {
		t.Fatal("destination collision accepted")
	}
	tm, err := from.Show("API")
	if err != nil || *tm.Definition != "local" {
		t.Fatalf("source changed on rejection: %+v %v", tm, err)
	}
	plan, err := PlanMove(repo, from, to, "API", true)
	if err != nil {
		t.Fatal(err)
	}
	if len(plan.Plan.Before) != 2 || len(plan.Plan.After) != 2 {
		t.Fatalf("move did not include both stores: %+v", plan.Plan)
	}
	if _, err := from.Show("API"); err != nil {
		t.Fatal("planning mutated source")
	}
}

func TestForcedMoveWithinSameScopeRetainsSingleTerm(t *testing.T) {
	repo := t.TempDir()
	scope := Scope{Root: repo, Kind: "project"}
	if _, err := scope.New("API", Input{}, nil, false); err != nil {
		t.Fatal(err)
	}
	move, err := PlanMove(repo, scope, scope, "API", true)
	if err != nil {
		t.Fatal(err)
	}
	if len(move.Plan.Watches) != 1 || len(move.Plan.After) != 1 {
		t.Fatalf("same-scope move plan: %+v", move.Plan)
	}
}

func TestScopedTermJSONRetainsIdentityAndScope(t *testing.T) {
	scope := "project"
	item := ScopedTerm{Term: model.Term{Term: "API"}, Identity: "API", Scope: &scope}
	raw, err := json.Marshal(item)
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]any
	if err := json.Unmarshal(raw, &fields); err != nil {
		t.Fatal(err)
	}
	if fields["identity"] != "API" || fields["scope"] != "project" || fields["term"] != "API" {
		t.Fatalf("promoted term marshaler hid view fields: %s", raw)
	}
}
