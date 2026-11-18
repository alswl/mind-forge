package model

import "testing"

func TestLegacyLifecycleModels(t *testing.T) {
	t.Run("reference_sorting_deterministic", func(t *testing.T) {
		line := uint64(42)
		refs := []Reference{{FromKind: "article", FromID: "zeta", FromPath: stringPointerForLegacy("docs/b.md"), Kind: "mention"}, {FromKind: "article", FromID: "alpha", FromPath: stringPointerForLegacy("docs/a.md"), Line: &line, Kind: "mention"}}
		SortReferences(refs)
		legacyConfigEqual(t, refs[0].FromID, "alpha")
		legacyConfigEqual(t, refs[1].FromID, "zeta")
	})
	t.Run("planned_change_sorting_deterministic", func(t *testing.T) {
		changes := []PlannedChange{{Op: "refresh_index", Path: "projects/foo/mind-index.yaml"}, {Op: "update_yaml", Path: "projects/foo/terms.yaml", Old: stringPointerForLegacy("TLA")}}
		SortPlannedChanges(changes)
		legacyConfigEqual(t, changes[0].Path, "projects/foo/mind-index.yaml")
		legacyConfigEqual(t, changes[1].Path, "projects/foo/terms.yaml")
	})
	t.Run("scope_ref_serialization", func(t *testing.T) {
		for _, scope := range []ScopeRef{{Project: stringPointerForLegacy("alpha")}, {Global: true}} {
			got := legacyDomainJSON(t, scope)
			legacyConfigEqual(t, got["global"], scope.Global)
			value, ok := got["project"]
			if !ok {
				t.Fatal("project key absent")
			}
			if scope.Global {
				legacyConfigEqual(t, value, nil)
			} else {
				legacyConfigEqual(t, value, "alpha")
			}
		}
	})
}

func TestPlannedChangesSortReversedPaths(t *testing.T) {
	changes := []PlannedChange{{Op: "update_yaml", Path: "projects/foo/terms.yaml"}, {Op: "refresh_index", Path: "projects/foo/mind-index.yaml"}}
	SortPlannedChanges(changes)
	if changes[0].Path != "projects/foo/mind-index.yaml" || changes[1].Path != "projects/foo/terms.yaml" {
		t.Fatalf("reversed changes not sorted: %+v", changes)
	}
}
