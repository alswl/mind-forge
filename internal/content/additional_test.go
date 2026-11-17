package content

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func additionalFixture(t *testing.T, layout string, file bool) (string, string, string, string) {
	t.Helper()
	root := canonicalPath(t.TempDir())
	project := filepath.Join(root, "p")
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects: ['./p']\n")
	putContent(t, project, "mind.yaml", "schema: '1'\ncontent_layout: "+layout+"\nminds: {a: {}, b: {}}\n")
	suffix := ""
	if file {
		suffix = ".md"
	}
	old, target := "evals/a"+suffix, "results/a"+suffix
	if layout == "bundle" {
		old, target = "a/evals"+suffix, "a/results"+suffix
	}
	if file {
		putContent(t, project, old, "# Author\r\n")
	} else {
		putContent(t, project, old+"/data.json", "{\"preserve\":true}\n")
	}
	return root, project, old, target
}

func TestAdditionalSelectorsAndTargetsKeepOwnerAndShape(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		for _, file := range []bool{false, true} {
			root, project, old, target := additionalFixture(t, layout, file)
			items, err := DiscoverAdditional(root, project)
			if err != nil || len(items) != 1 {
				t.Fatalf("%v %v", items, err)
			}
			for _, selector := range []string{old, "evals", "a::additional::evals", "p::a::additional::evals"} {
				if got, err := SelectAdditional(items, selector); err != nil || got != items[0] {
					t.Fatalf("selector %s: %#v %v", selector, got, err)
				}
			}
			if kind, err := AdditionalRenameTarget(root, project, items[0], target, items); err != nil || kind != "results" {
				t.Fatalf("valid target: %s %v", kind, err)
			}
			for _, bad := range []string{old, "results/b.md", "b/results", "a/results/x", "a/prompt.md", "a/assets", "outputs/a", "../results/a", "/results/a", "results/a/", "a/.hidden"} {
				if _, err := AdditionalRenameTarget(root, project, items[0], bad, items); err == nil {
					t.Errorf("accepted invalid target %s (%s file=%v)", bad, layout, file)
				}
			}
			parent := strings.Split(target, "/")[0]
			if layout == "category" {
				putContent(t, project, strings.ToUpper(parent)+"/b.md", "case conflict")
				if _, err := AdditionalRenameTarget(root, project, items[0], target, items); err == nil {
					t.Fatal("case-colliding parent accepted")
				}
			}
		}
	}
	items := []Additional{{Identity: "p::a::additional::evals", Mind: "a", AdditionalKind: "evals"}, {Identity: "p::b::additional::evals", Mind: "b", AdditionalKind: "evals"}}
	_, err := SelectAdditional(items, "evals")
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "selection_ambiguous" || failure.Candidates == nil || len(*failure.Candidates) != 2 {
		t.Fatal(err)
	}
	for _, selector := range []string{"../missing", "/abs/missing", "./missing"} {
		_, err := SelectAdditional(nil, selector)
		if !errors.As(err, &failure) || failure.Kind != "unsafe_path" {
			t.Fatalf("traversal %s: %v", selector, err)
		}
	}
}

func TestAdditionalPlanKeepsAuthorityAndRewritesReferences(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		for _, file := range []bool{false, true} {
			root, project, old, target := additionalFixture(t, layout, file)
			payload := old
			newPayload := target
			if !file {
				payload += "/data.json"
				newPayload += "/data.json"
			}
			reference := "outputs/a/link.md"
			if layout == "bundle" {
				reference = "a/outputs/link.md"
			}
			putContent(t, project, reference, "# Literal\r\n[report](../../"+payload+"#part)\r\n`[code](../../"+payload+")`\r\n")
			index := "schema: '1'\nsources:\n  exact:\n    name: exact\n    path: " + payload + "\n    opaque: {flags: [one, two]}\nassets:\n  " + payload + ":\n    path: " + payload + "\n    opaque: 7\nterms: [opaque] # untouched bytes\nextra: [retained] # literal\n"
			putContent(t, project, "mind-index.yaml", index)
			if err := os.Chmod(filepath.Join(project, "mind-index.yaml"), 0o600); err != nil {
				t.Fatal(err)
			}
			before, err := lifecycle.SnapshotFiles(root, []string{"."})
			if err != nil {
				t.Fatal(err)
			}
			plan, err := PlanAdditional(root, project, "p::a::additional::evals", &target)
			if err != nil {
				t.Fatal(err)
			}
			after := plan.After
			if _, exists := after["p/"+old]; exists || !reflect.DeepEqual(after["p/"+newPayload], before["p/"+payload]) {
				t.Fatal("payload not moved intact")
			}
			newLink, err := filepath.Rel(filepath.Dir(reference), newPayload)
			if err != nil {
				t.Fatal(err)
			}
			want := "# Literal\r\n[report](" + filepath.ToSlash(newLink) + "#part)\r\n`[code](../../" + payload + ")`\r\n"
			if string(after["p/"+reference].Bytes) != want {
				t.Fatalf("reference edit: %s", after["p/"+reference].Bytes)
			}
			indexEntry := after["p/mind-index.yaml"]
			if indexEntry.Mode != 0o600 || !strings.Contains(string(indexEntry.Bytes), "terms: [opaque] # untouched bytes\nextra: [retained] # literal\n") {
				t.Fatal("unrelated bytes or mode changed")
			}
			doc, err := store.ParseYAML(indexEntry.Bytes)
			if err != nil {
				t.Fatal(err)
			}
			value, err := model.JSONFromYAML(doc.Root.Content[0])
			if err != nil {
				t.Fatal(err)
			}
			data := value.(map[string]any)
			row := data["sources"].(map[string]any)["exact"].(map[string]any)
			if row["path"] != newPayload || row["opaque"] == nil {
				t.Fatalf("authority lost: %v", row)
			}
			if _, exists := data["assets"].(map[string]any)[newPayload]; !exists {
				t.Fatal("path-key authority not renamed")
			}
			extension := data["additional"].([]any)[0].(map[string]any)
			if extension["identity"] != "p::a::additional::results" || extension["path"] != target || extension["project_identity"] != "p" {
				t.Fatalf("projection: %v", extension)
			}
			current, err := lifecycle.SnapshotFiles(root, []string{"."})
			if err != nil || !reflect.DeepEqual(current, before) {
				t.Fatal("planning wrote files")
			}
		}
	}
}

func TestAdditionalRemovalRefusesInboundReferencesAndRetainsOpaqueSections(t *testing.T) {
	root, project, old, _ := additionalFixture(t, "category", false)
	putContent(t, root, "other/link.md", "[inbound](../p/"+old+"/data.json)\n")
	_, err := PlanAdditional(root, project, old, nil)
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "inbound_reference" {
		t.Fatal(err)
	}
	putContent(t, root, "other/link.md", "`[masked](../p/"+old+"/data.json)`\n")
	putContent(t, project, "mind-index.yaml", "schema: '1'\nopaque: [keep] # literal\nsources:\n  sample: {path: "+old+"/data.json, unknown: preserve}\n")
	plan, err := PlanAdditional(root, project, old, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, exists := plan.After["p/"+old]; exists || !strings.Contains(string(plan.After["p/mind-index.yaml"].Bytes), "opaque: [keep] # literal") {
		t.Fatal("remove plan lost opaque content or retained resource")
	}
	for _, bad := range []string{"sources: {sample: {unknown: value}}", "assets: {sample: {path: missing/file}}", "articles: {sample: {title: NoPath}}"} {
		putContent(t, project, "mind-index.yaml", "schema: '1'\n"+bad+"\n")
		_, err := PlanAdditional(root, project, old, nil)
		if !errors.As(err, &failure) || failure.Kind != "ownership_unresolved" {
			t.Fatalf("accepted unresolved authority %s: %v", bad, err)
		}
	}
}

func TestAdditionalPlanningValidatesFullOwnershipBeyondQueryScan(t *testing.T) {
	root, project, old, target := additionalFixture(t, "category", false)
	putContent(t, project, "docs/unowned.md", "# Legacy author\n")
	if items, err := DiscoverAdditional(root, project); err != nil || len(items) != 1 {
		t.Fatalf("query unexpectedly rejected legacy docs: %v %v", items, err)
	}
	_, err := PlanAdditional(root, project, old, &target)
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "ownership_unresolved" {
		t.Fatalf("full planning accepted unknown ownership: %v", err)
	}
}

func TestLifecycleSectionEditDetachesDependentAliasesAndSupportsFlowKeys(t *testing.T) {
	for _, raw := range []string{
		"schema: '1'\nsources:\n  selected: &saved {path: old/file, unknown: retain}\nopaque: *saved\nterms: [keep] # untouched\n",
		"{schema: '1', sources: {selected: {path: old/file, unknown: retain}}, opaque: {path: old/file}}\n",
		"schema: '1'\n'sources': {selected: {path: old/file, unknown: retain}}\nopaque: {path: old/file}\n",
	} {
		updated, err := spliceLifecycleSection([]byte(raw), "sources", map[string]any{"selected": map[string]any{"path": "new/file", "unknown": "retain"}})
		if err != nil {
			t.Fatal(err)
		}
		doc, err := store.ParseYAML(updated)
		if err != nil {
			t.Fatalf("invalid result: %s: %v", updated, err)
		}
		value, err := model.JSONFromYAML(doc.Root.Content[0])
		if err != nil {
			t.Fatal(err)
		}
		data := value.(map[string]any)
		if data["opaque"].(map[string]any)["path"] != "old/file" || data["sources"].(map[string]any)["selected"].(map[string]any)["path"] != "new/file" {
			t.Fatalf("aliased registration value changed: %v", data)
		}
		if strings.Contains(raw, "# untouched") && !strings.Contains(string(updated), "terms: [keep] # untouched\n") {
			t.Fatalf("unrelated section was reformatted: %s", updated)
		}
	}
}
