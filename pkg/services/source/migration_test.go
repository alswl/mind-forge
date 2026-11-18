package source

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestFrozenSourceRegisterOnlyKeepsBytesAndRepeatedRegistration(t *testing.T) {
	project := sourceProject(t)
	input := putSource(t, project, "sources/file/synthetic.md", "synthetic source bytes\n")
	service := fixedService()
	for i := 0; i < 2; i++ {
		outcome, err := service.Add(context.Background(), filepath.Dir(project), project, filepath.Dir(project), input, AddOptions{RegisterOnly: true})
		if err != nil || outcome.Mode != "register" || outcome.Replaced {
			t.Fatalf("%#v %v", outcome, err)
		}
		raw, err := os.ReadFile(input)
		if err != nil || string(raw) != "synthetic source bytes\n" {
			t.Fatal("registration touched source bytes")
		}
		document, err := readIndex(project)
		if err != nil {
			t.Fatal(err)
		}
		items := sourceEntries(document.Value)
		if len(items) != 1 || items[0].Path == nil || *items[0].Path != "sources/file/synthetic.md" {
			t.Fatalf("%#v", items)
		}
	}
}

func TestFrozenSourceIndexKeepsExistingAndUnknownMissingRegistrations(t *testing.T) {
	for _, missing := range []bool{false, true} {
		t.Run(map[bool]string{false: "retains_existing_and_adds_disk_sources", true: "keeps_missing_file_entries_and_uninterpreted_fields"}[missing], func(t *testing.T) {
			project := sourceProject(t)
			putSource(t, project, "sources/file/kept.md", "kept\n")
			index := "schema: '1'\nsources:\n  kept:\n    name: kept\n    path: sources/file/kept.md\n    added_at: old\n    updated_at: old\n    provenance_note: keep-me-verbatim\n"
			if missing {
				index += "  ghost:\n    name: ghost\n    path: sources/file/ghost.md\n    added_at: gone\n    updated_at: gone\n"
			} else {
				putSource(t, project, "sources/file/added.md", "added\n")
			}
			putSource(t, project, "mind-index.yaml", index)
			report, err := fixedService().Reconcile(project, false)
			if err != nil || report.KeptCount != 1 {
				t.Fatalf("%#v %v", report, err)
			}
			document, err := readIndex(project)
			if err != nil {
				t.Fatal(err)
			}
			items := sourceEntries(document.Value)
			if len(items) != 2 || items[0].Name != "kept" || items[0].AddedAt != "old" || items[0].Extra.Get("provenance_note") == nil {
				t.Fatalf("%#v", items)
			}
			if missing {
				if len(report.Removed) != 1 || report.Removed[0].Name != "ghost" || items[1].Name != "ghost" || items[1].AddedAt != "gone" {
					t.Fatalf("%#v %#v", report, items)
				}
				raw, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml"))
				if !strings.Contains(string(raw), "provenance_note: keep-me-verbatim") {
					t.Fatal("lost uninterpreted field")
				}
			} else if len(report.Added) != 1 || len(report.Removed) != 0 {
				t.Fatalf("%#v", report)
			}
		})
	}
}

func TestFrozenSourceIndexNoopOrderAndAppend(t *testing.T) {
	project := sourceProject(t)
	for _, name := range []string{"zzz", "aaa"} {
		putSource(t, project, "sources/file/"+name+".md", name)
	}
	putSource(t, project, "mind-index.yaml", "schema: '1'\nsources:\n  zzz: {name: zzz, path: sources/file/zzz.md, added_at: t, updated_at: t}\n  aaa: {name: aaa, path: sources/file/aaa.md, added_at: t, updated_at: t}\n")
	service := fixedService()
	for _, want := range [][]string{{"zzz", "aaa"}, {"zzz", "aaa", "mmm"}} {
		if len(want) == 3 {
			putSource(t, project, "sources/file/mmm.md", "m")
		}
		report, err := service.Reconcile(project, false)
		if err != nil || len(report.Added) != len(want)-2 || len(report.Removed) != 0 {
			t.Fatalf("%#v %v", report, err)
		}
		document, _ := readIndex(project)
		names := []string{}
		for _, item := range sourceEntries(document.Value) {
			names = append(names, item.Name)
		}
		if !reflect.DeepEqual(names, want) {
			t.Fatalf("%v want%v", names, want)
		}
	}
}

func TestFrozenSourceMoveUpdatesBothProjectIndexes(t *testing.T) {
	source, target := sourceProject(t), sourceProject(t)
	for _, path := range []string{filepath.Join(source, "mind.yaml"), filepath.Join(target, "mind.yaml"), filepath.Join(target, "mind-index.yaml")} {
		if err := os.Remove(path); err != nil {
			t.Fatal(err)
		}
	}
	putSource(t, source, "sources/file/note.md", "note\n")
	putSource(t, source, "mind-index.yaml", "schema: '1'\nsources:\n- name: note\n  type: file\n  path: sources/file/note.md\n")
	report, err := fixedService().Move(source, target, "note", false)
	if err != nil || report.NewPath != "sources/file/note.md" {
		t.Fatalf("%#v %v", report, err)
	}
	if _, err := os.Stat(filepath.Join(source, report.OldPath)); !os.IsNotExist(err) {
		t.Fatal("old file retained")
	}
	raw, err := os.ReadFile(filepath.Join(target, report.NewPath))
	if err != nil || string(raw) != "note\n" {
		t.Fatal("target evidence missing")
	}
	before, _ := readIndex(source)
	after, _ := readIndex(target)
	if len(sourceEntries(before.Value)) != 0 || len(sourceEntries(after.Value)) != 1 || sourceEntries(after.Value)[0].Name != "note" {
		t.Fatal("move registration mismatch")
	}
}
