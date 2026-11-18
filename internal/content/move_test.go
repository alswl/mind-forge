package content

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
)

func lifecycleFixture(t *testing.T, mode string) (string, string) {
	t.Helper()
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects: ['./p', './q']\n")
	project := filepath.Join(root, "p")
	putContent(t, project, "mind.yaml", "schema: '1'\ncontent_layout: "+mode+"\nminds:\n  a: {title: Author title, default_output: one, unknown: [retain]}\n  zero: {title: No outputs}\nopaque: [preserve] # exact\n")
	putContent(t, root, "q/mind.yaml", "schema: '1'\ncontent_layout: "+mode+"\nminds: {b: {title: Receiver}}\n")
	putContent(t, root, "q/mind-index.yaml", "schema: '1'\nopaque: [destination] # exact\n")
	outputRoot := lifecycle.OwnedDestination(mode, "a", "outputs", false)
	asset := lifecycle.OwnedDestination(mode, "a", "assets", false) + "/x.png"
	source := lifecycle.OwnedDestination(mode, "a", "sources", false) + "/e.md"
	relative, err := filepath.Rel(outputRoot, asset)
	if err != nil {
		t.Fatal(err)
	}
	putContent(t, project, outputRoot+"/one.md", "# Original title\r\n![image]("+filepath.ToSlash(relative)+"?v=1#p \"title\")\r\n")
	putContent(t, project, outputRoot+"/two/01.md", "# Directory author\n")
	putContent(t, project, asset, string([]byte{0, 255, 3}))
	putContent(t, project, source, "# Source\n")
	putContent(t, project, lifecycle.OwnedDestination(mode, "a", "prompt", true), "# Prompt\n")
	putContent(t, project, lifecycle.OwnedDestination(mode, "a", "thinking", true), "# Thinking\n")
	putContent(t, project, lifecycle.OwnedDestination(mode, "a", "evals", false)+"/data.json", "{\"keep\":true}\n")
	putContent(t, project, "reader.md", "[read]("+outputRoot+"/one.md?q=1#p \"label\")\n")
	putContent(t, project, "mind-index.yaml", "schema: '1'\nsources:\n  evidence: {name: evidence, path: "+source+", unknown: [keep], project: p}\nassets:\n  image: {name: image, path: "+asset+", unknown: {retain: true}}\npublish_records:\n  record: {path: "+outputRoot+"/one.md, project: p, unknown: [keep]}\nopaque: [literal] # exact\n")
	return root, project
}

func plannedText(t *testing.T, plan lifecycle.Plan, name string) string {
	t.Helper()
	entry, exists := plan.After[name]
	if !exists || entry.Kind != "file" {
		t.Fatalf("missing planned file %s", name)
	}
	return string(entry.Bytes)
}

func TestLifecycleConversionPlansEveryOwnedRoleAndRetainsPlaceholders(t *testing.T) {
	for _, mode := range []string{"category", "bundle"} {
		root, project := lifecycleFixture(t, mode)
		target := "bundle"
		if mode == "bundle" {
			target = "category"
			putContent(t, project, "a/.keep", "hidden container placeholder\n")
		}
		before, err := lifecycle.SnapshotFiles(root, []string{"."})
		if err != nil {
			t.Fatal(err)
		}
		plan, err := PlanLifecycle(root, project, LifecycleOperation{Kind: "convert", Layout: target})
		if err != nil {
			t.Fatal(err)
		}
		for _, role := range []struct {
			kind  string
			file  bool
			child string
		}{{"outputs", false, "/one.md"}, {"outputs", false, "/two/01.md"}, {"assets", false, "/x.png"}, {"sources", false, "/e.md"}, {"prompt", true, ""}, {"thinking", true, ""}, {"evals", false, "/data.json"}} {
			old := "p/" + lifecycle.OwnedDestination(mode, "a", role.kind, role.file) + role.child
			new := "p/" + lifecycle.OwnedDestination(target, "a", role.kind, role.file) + role.child
			if _, exists := plan.After[new]; !exists {
				t.Fatalf("role not converted: %s -> %s", old, new)
			}
			if _, exists := plan.After[old]; exists {
				t.Fatalf("old role retained: %s", old)
			}
			if role.kind != "outputs" && !reflect.DeepEqual(plan.After[new], before[old]) {
				t.Fatalf("role bytes/mode changed: %s", new)
			}
		}
		if mode == "bundle" && plannedText(t, plan, "p/a/.keep") != "hidden container placeholder\n" {
			t.Fatal("hidden placeholder lost")
		}
		config, err := snapshotMetadata(plan.After, "p/mind.yaml")
		if err != nil {
			t.Fatal(err)
		}
		if actual, _ := yamlText(yamlField(config, "content_layout")); actual != target {
			t.Fatal("layout not changed")
		}
		if yamlField(yamlField(config, "minds"), "zero") == nil {
			t.Fatal("zero-output registration lost")
		}
		for _, name := range []string{"p/mind.yaml", "p/mind-index.yaml", "q/mind-index.yaml"} {
			if name != "q/mind-index.yaml" && !strings.Contains(plannedText(t, plan, name), "# exact\n") {
				t.Fatalf("opaque bytes lost in %s", name)
			}
		}
		index, err := snapshotMetadata(plan.After, "p/mind-index.yaml")
		if err != nil {
			t.Fatal(err)
		}
		row := yamlField(yamlField(index, "sources"), "evidence")
		if actual, _ := yamlText(yamlField(row, "path")); actual != lifecycle.OwnedDestination(target, "a", "sources", false)+"/e.md" {
			t.Fatalf("stale source %s", actual)
		}
		current, err := lifecycle.SnapshotFiles(root, []string{"."})
		if err != nil || !reflect.DeepEqual(current, before) {
			t.Fatal("planning wrote files")
		}
	}
}

func TestMindRenameAndMoveCarryResourcesRegistrationAndAuthority(t *testing.T) {
	for _, mode := range []string{"category", "bundle"} {
		for _, targetMode := range []string{"category", "bundle"} {
			for _, rename := range []bool{false, true} {
				root, project := lifecycleFixture(t, mode)
				putContent(t, root, "q/mind.yaml", "schema: '1'\ncontent_layout: "+targetMode+"\nminds: {b: {title: Receiver}}\n")
				op := LifecycleOperation{Kind: "mind-move", Identity: "p::a", Project: "./q"}
				dest, name := "q", "a"
				destMode := targetMode
				if rename {
					op = LifecycleOperation{Kind: "mind-rename", Identity: "p::a", Name: "renamed"}
					dest, name, destMode = "p", "renamed", mode
				}
				plan, err := PlanLifecycle(root, project, op)
				if err != nil {
					t.Fatalf("%s->%s rename=%v: %v", mode, targetMode, rename, err)
				}
				cfg, err := snapshotMetadata(plan.After, dest+"/mind.yaml")
				if err != nil {
					t.Fatal(err)
				}
				registration := yamlField(yamlField(cfg, "minds"), name)
				if title, _ := yamlText(yamlField(registration, "title")); title != "Author title" || yamlField(registration, "unknown") == nil {
					t.Fatal("mind registration lost")
				}
				index, err := snapshotMetadata(plan.After, dest+"/mind-index.yaml")
				if err != nil {
					t.Fatal(err)
				}
				for _, section := range []string{"sources", "assets", "publish_records"} {
					rows, err := authorityRows(yamlField(index, section))
					if err != nil || len(rows) != 1 {
						t.Fatalf("authority %s %v %v", section, rows, err)
					}
					if yamlField(rows[0].value, "unknown") == nil {
						t.Fatal("opaque registration dropped")
					}
					if actual, _ := yamlText(yamlField(rows[0].value, "path")); strings.Contains(actual, "/a/") && name != "a" {
						t.Fatalf("stale registration %s", actual)
					}
				}
				newOutput := dest + "/" + lifecycle.OwnedDestination(destMode, name, "outputs", false) + "/one.md"
				if !strings.Contains(plannedText(t, plan, newOutput), "# Original title\r\n") {
					t.Fatal("author title changed")
				}
				if rename {
					if !strings.Contains(plannedText(t, plan, "p/reader.md"), "renamed") {
						t.Fatal("inbound link not renamed")
					}
				} else {
					if !strings.Contains(plannedText(t, plan, "p/reader.md"), "../q/") {
						t.Fatal("cross-project link not rebased")
					}
					sourceIndex, err := snapshotMetadata(plan.After, "p/mind-index.yaml")
					if err != nil {
						t.Fatal(err)
					}
					rows, _ := authorityRows(yamlField(sourceIndex, "sources"))
					if len(rows) != 0 {
						t.Fatal("source authority duplicated")
					}
				}
			}
		}
	}
}

func TestOutputLifecycleKeepsShapeUpdatesDefaultAndMovesPublishRecords(t *testing.T) {
	for _, mode := range []string{"category", "bundle"} {
		for _, targetMode := range []string{"category", "bundle"} {
			for _, kind := range []string{"output-rename", "output-move", "output-remove"} {
				for _, name := range []string{"one", "two"} {
					root, project := lifecycleFixture(t, mode)
					putContent(t, root, "q/mind.yaml", "schema: '1'\ncontent_layout: "+targetMode+"\nminds: {b: {title: Receiver}}\n")
					op := LifecycleOperation{Kind: kind, Identity: "p::a::" + name, Name: "renamed", Mind: "b", Project: "./q"}
					if kind == "output-remove" {
						if err := os.Remove(filepath.Join(project, "reader.md")); err != nil {
							t.Fatal(err)
						}
					}
					plan, err := PlanLifecycle(root, project, op)
					if err != nil {
						t.Fatalf("%s %s %s %s: %v", mode, targetMode, kind, name, err)
					}
					cfg, err := snapshotMetadata(plan.After, "p/mind.yaml")
					if err != nil {
						t.Fatal(err)
					}
					selected := yamlField(yamlField(yamlField(cfg, "minds"), "a"), "default_output")
					if name == "one" {
						if kind == "output-rename" {
							if value, _ := yamlText(selected); value != "renamed" {
								t.Fatal("default not renamed")
							}
						} else if selected.Tag != "!!null" {
							t.Fatal("default not cleared")
						}
					}
					if kind == "output-remove" {
						continue
					}
					dest, owner, newName, newMode := "p", "a", "renamed", mode
					if kind == "output-move" {
						dest, owner, newName, newMode = "q", "b", name, targetMode
					}
					file := "/" + newName + "/01.md"
					if name == "one" {
						file = "/" + newName + ".md"
					}
					if text := plannedText(t, plan, dest+"/"+lifecycle.OwnedDestination(newMode, owner, "outputs", false)+file); !strings.Contains(text, "title") && !strings.Contains(text, "Directory author") {
						t.Fatal("wrong author")
					}
					if kind == "output-move" && name == "one" {
						index, err := snapshotMetadata(plan.After, "q/mind-index.yaml")
						if err != nil {
							t.Fatal(err)
						}
						row := yamlField(yamlField(index, "publish_records"), "record")
						if value, _ := yamlText(yamlField(row, "project")); value != "q" || yamlField(row, "unknown") == nil {
							t.Fatal("record not transferred")
						}
					}
				}
			}
		}
	}
}

func TestLifecycleRefusalsAreBeforeWritesAndDeletionIgnoresInternalLinks(t *testing.T) {
	for _, mode := range []string{"category", "bundle"} {
		root, project := lifecycleFixture(t, mode)
		for _, op := range []LifecycleOperation{{Kind: "mind-rename", Identity: "p::a", Name: "a"}, {Kind: "output-rename", Identity: "p::a::one", Name: "two"}, {Kind: "convert", Layout: mode}, {Kind: "mind-remove", Identity: "p::a"}, {Kind: "output-remove", Identity: "p::a::one"}} {
			before, err := lifecycle.SnapshotFiles(root, []string{"."})
			if err != nil {
				t.Fatal(err)
			}
			_, err = PlanLifecycle(root, project, op)
			var failure *output.Error
			if !errors.As(err, &failure) {
				t.Fatalf("accepted %v: %v", op, err)
			}
			if strings.Contains(op.Kind, "remove") && failure.Kind != "inbound_reference" {
				t.Fatalf("wrong deletion refusal %v", err)
			}
			current, err := lifecycle.SnapshotFiles(root, []string{"."})
			if err != nil || !reflect.DeepEqual(current, before) {
				t.Fatal("refusal wrote files")
			}
		}
		if err := os.Remove(filepath.Join(project, "reader.md")); err != nil {
			t.Fatal(err)
		}
		outputRoot := lifecycle.OwnedDestination(mode, "a", "outputs", false)
		putContent(t, project, outputRoot+"/two/self.md", "[internal](../one.md)\n")
		plan, err := PlanLifecycle(root, project, LifecycleOperation{Kind: "mind-remove", Identity: "p::a"})
		if err != nil {
			t.Fatal(err)
		}
		if _, exists := plan.After["p/"+outputRoot+"/one.md"]; exists {
			t.Fatal("author not removed")
		}
		cfg, err := snapshotMetadata(plan.After, "p/mind.yaml")
		if err != nil {
			t.Fatal(err)
		}
		if yamlField(yamlField(cfg, "minds"), "a") != nil || yamlField(yamlField(cfg, "minds"), "zero") == nil {
			t.Fatal("wrong registrations removed")
		}
	}
}

func TestMindMoveRefusesDuplicateAuthorityKeyAndHiddenBundleOrphans(t *testing.T) {
	root, project := lifecycleFixture(t, "bundle")
	putContent(t, root, "q/mind.yaml", "schema: '1'\ncontent_layout: category\nminds: {b: {}}\n")
	putContent(t, project, "a/.private", "container-owned hidden file\n")
	_, err := PlanLifecycle(root, project, LifecycleOperation{Kind: "mind-move", Identity: "p::a", Project: "./q"})
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "ownership_unresolved" {
		t.Fatalf("orphan allowed: %v", err)
	}
	if err := os.Remove(filepath.Join(project, "a/.private")); err != nil {
		t.Fatal(err)
	}
	putContent(t, root, "q/sources/b/e.md", "# Existing\n")
	putContent(t, root, "q/mind-index.yaml", "schema: '1'\nsources:\n  evidence: {path: sources/b/e.md, unknown: preserve}\n")
	_, err = PlanLifecycle(root, project, LifecycleOperation{Kind: "mind-move", Identity: "p::a", Project: "./q"})
	if !errors.As(err, &failure) || failure.Kind != "path_conflict" {
		t.Fatalf("duplicate authority key allowed: %v", err)
	}
}

func TestRootLifecycleConversionDoesNotClaimChildProject(t *testing.T) {
	root, project := lifecycleFixture(t, "category")
	entries, err := os.ReadDir(project)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if err := os.Rename(filepath.Join(project, entry.Name()), filepath.Join(root, entry.Name())); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Remove(project); err != nil {
		t.Fatal(err)
	}
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects: ['.', './q']\n")
	before, err := lifecycle.SnapshotFiles(root, []string{"."})
	if err != nil {
		t.Fatal(err)
	}
	plan, err := PlanLifecycle(root, root, LifecycleOperation{Kind: "convert", Layout: "bundle"})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(plannedText(t, plan, "a/outputs/one.md"), "# Original title") {
		t.Fatal("root author not converted")
	}
	if !reflect.DeepEqual(plan.After["q/mind.yaml"], before["q/mind.yaml"]) || !reflect.DeepEqual(plan.After["q/mind-index.yaml"], before["q/mind-index.yaml"]) {
		t.Fatal("child project modified")
	}
	index, err := snapshotMetadata(plan.After, "mind-index.yaml")
	if err != nil {
		t.Fatal(err)
	}
	rows, err := authorityRows(yamlField(index, "outputs"))
	if err != nil || len(rows) != 2 {
		t.Fatalf("root projection %v %v", rows, err)
	}
	if id, _ := yamlText(yamlField(rows[0].value, "identity")); !strings.HasPrefix(id, "::a::") {
		t.Fatal("root identity corrupted")
	}
}
