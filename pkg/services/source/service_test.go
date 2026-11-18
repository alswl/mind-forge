package source

import (
	"bytes"
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func putSource(t *testing.T, root, name, value string) string {
	t.Helper()
	path := filepath.Join(root, name)
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(value), 0644); err != nil {
		t.Fatal(err)
	}
	return path
}

func sourceProject(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	putSource(t, root, "mind.yaml", "schema: '1'\n")
	putSource(t, root, "mind-index.yaml", "schema: '1'\nterms: [] # exact\n")
	return root
}

func fixedService() Service {
	return Service{Now: func() time.Time { return time.Date(2026, 1, 2, 3, 4, 5, 0, time.UTC) }}
}

func TestLocalImportRegistrationForceAndLinkPreserveEvidence(t *testing.T) {
	project := sourceProject(t)
	input := putSource(t, t.TempDir(), "report.PDF", "author bytes\r\n")
	if err := os.Chmod(input, 0640|os.ModeSetuid); err != nil {
		t.Fatal(err)
	}
	service := fixedService()
	outcome, err := service.Add(context.Background(), project, project, project, input, AddOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if outcome.Source.Kind != "pdf" || outcome.Source.Name != "report" || outcome.Mode != "copy" {
		t.Fatalf("%#v", outcome)
	}
	stored := joinPath(project, *outcome.Source.Path)
	if info, err := os.Stat(stored); err != nil || info.Mode()&(os.ModePerm|os.ModeSetuid) != (0640|os.ModeSetuid) {
		t.Fatalf("copy lost source permissions: %v %v", info, err)
	}
	before, err := os.ReadFile(stored)
	if err != nil {
		t.Fatal(err)
	}
	registered, err := service.Add(context.Background(), project, project, project, stored, AddOptions{RegisterOnly: true})
	if err != nil || registered.Source.UpdatedAt != outcome.Source.UpdatedAt {
		t.Fatalf("%#v %v", registered, err)
	}
	if _, err := service.Add(context.Background(), project, project, project, stored, AddOptions{}); err == nil {
		t.Fatal("copied an already owned file")
	}
	name := "report"
	other := putSource(t, t.TempDir(), "replacement.pdf", "replacement bytes")
	document, _ := readIndex(project)
	entries := sourceEntries(document.Value)
	entries[0].Tags = []string{"retain"}
	updated := document.Value
	updated.Sources = &entries
	if _, err := service.save(project, document, updated); err != nil {
		t.Fatal(err)
	}
	forced, err := service.Add(context.Background(), project, project, project, other, AddOptions{Name: &name, Force: true})
	if err != nil {
		t.Fatal(err)
	}
	if !forced.Replaced || forced.ReplacedLocation == nil || forced.Source.AddedAt != outcome.Source.AddedAt || len(forced.Source.Tags) != 1 {
		t.Fatalf("lost metadata: %#v", forced)
	}
	if _, err := os.Stat(stored); !os.IsNotExist(err) {
		t.Fatal("old evidence not cleaned")
	}
	if !bytes.Equal(before, []byte("author bytes\r\n")) {
		t.Fatal("copy changed source")
	}
	linkInput := putSource(t, t.TempDir(), "linked.md", "linked author")
	linked, err := service.Add(context.Background(), project, project, project, linkInput, AddOptions{Link: true})
	if err != nil {
		t.Fatal(err)
	}
	canonicalInput, _ := filepath.EvalSymlinks(linkInput)
	if target, err := os.Readlink(joinPath(project, *linked.Source.Path)); err != nil || target != canonicalInput {
		t.Fatalf("%s %v", target, err)
	}
	if _, err := service.Add(context.Background(), project, project, project, linkInput, AddOptions{Force: true}); err != nil {
		t.Fatalf("force could not replace owned symlink: %v", err)
	}
	if info, err := os.Lstat(joinPath(project, *linked.Source.Path)); err != nil || !info.Mode().IsRegular() {
		t.Fatal("owned link not replaced by a copy")
	}
	if raw, err := os.ReadFile(linkInput); err != nil || string(raw) != "linked author" {
		t.Fatal("link target was modified")
	}
	raw, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml"))
	if !strings.Contains(string(raw), "terms: [] # exact\n") {
		t.Fatal("unrelated section changed")
	}
}

func TestURLConflictPreflightAndForcedMetadata(t *testing.T) {
	project := sourceProject(t)
	service := fixedService()
	calls := 0
	service.Acquire = func(context.Context, string, AcquisitionOptions) ([]byte, error) {
		calls++
		return []byte("raw response"), nil
	}
	name := "evidence"
	first, err := service.Add(context.Background(), project, project, project, "https://example.test/first.html", AddOptions{Name: &name})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Add(context.Background(), project, project, project, "https://example.test/second.html", AddOptions{Name: &name}); err == nil || calls != 1 {
		t.Fatalf("conflict acquired replacement: calls=%d err=%v", calls, err)
	}
	second, err := service.Add(context.Background(), project, project, project, "https://example.test/second.html", AddOptions{Name: &name, Force: true})
	if err != nil {
		t.Fatal(err)
	}
	if calls != 2 || !second.Replaced || first.Source.AddedAt != second.Source.AddedAt || second.ReplacedLocation == nil {
		t.Fatalf("%#v calls%d", second, calls)
	}
}

func TestSourceFileMutationsRollbackWhenIndexPublicationFails(t *testing.T) {
	project := sourceProject(t)
	old := putSource(t, project, "sources/file/old.md", "old evidence")
	target := putSource(t, project, "sources/file/new.md", "replaced evidence")
	putSource(t, project, "mind-index.yaml", "schema: '1'\nsources:\n  old: {name: old, type: file, path: sources/file/old.md, unknown: retain}\nterms: [] # literal\n")
	before, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml"))
	sentinel := errors.New("index fault")
	service := Service{WriteIndex: func(string, []byte) error { return sentinel }}
	if _, err := service.Rename(project, project, "old", target, true, false); !errors.Is(err, sentinel) {
		t.Fatal(err)
	}
	for name, want := range map[string]string{old: "old evidence", target: "replaced evidence"} {
		raw, err := os.ReadFile(name)
		if err != nil || string(raw) != want {
			t.Fatalf("lost %s %q %v", name, raw, err)
		}
	}
	if _, err := service.Remove(project, "old", false, false, false); !errors.Is(err, sentinel) {
		t.Fatal(err)
	}
	if raw, err := os.ReadFile(old); err != nil || string(raw) != "old evidence" {
		t.Fatal("failed removal lost author")
	}
	if raw, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml")); !bytes.Equal(raw, before) {
		t.Fatal("failed metadata was published")
	}
	other := sourceProject(t)
	service.WriteIndex = func(name string, raw []byte) error {
		if strings.HasPrefix(name, other+string(filepath.Separator)) {
			return sentinel
		}
		return (store.AtomicWriter{}).Write(name, raw, 0644)
	}
	if _, err := service.Move(project, other, "old", false); !errors.Is(err, sentinel) {
		t.Fatal(err)
	}
	if raw, err := os.ReadFile(old); err != nil || string(raw) != "old evidence" {
		t.Fatal("failed cross-project move lost author")
	}
	if raw, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml")); !bytes.Equal(raw, before) {
		t.Fatal("failed cross-project move changed source index")
	}
}

func TestIndexKeepsMissingAuthorityOrderAndOnlyCleanDropsIt(t *testing.T) {
	project := sourceProject(t)
	putSource(t, project, "sources/file/z.md", "z")
	putSource(t, project, "sources/file/new.md", "new")
	putSource(t, project, "mind-index.yaml", "schema: '1'\nsources:\n  z: {name: z, path: sources/file/z.md, added_at: first, updated_at: previous, unknown: preserve}\n  missing: {name: missing, path: sources/file/missing.md, added_at: old}\nterms: [] # exact\n")
	service := fixedService()
	report, err := service.Reconcile(project, false)
	if err != nil {
		t.Fatal(err)
	}
	if len(report.Removed) != 1 || len(report.Added) != 1 || report.KeptCount != 1 {
		t.Fatalf("%#v", report)
	}
	doc, _ := readIndex(project)
	items := sourceEntries(doc.Value)
	if len(items) != 3 || items[0].Name != "z" || items[1].Name != "missing" || items[2].Name != "new" || items[0].AddedAt != "first" || items[0].Extra.Get("unknown") == nil {
		t.Fatalf("authority lost/reordered: %#v", items)
	}
	clean, err := service.Clean(project, false)
	if err != nil || len(clean.Removed) != 1 {
		t.Fatalf("%#v %v", clean, err)
	}
	doc, _ = readIndex(project)
	if len(sourceEntries(doc.Value)) != 2 {
		t.Fatal("clean retained missing registration")
	}
}
