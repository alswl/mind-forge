package asset

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func putAsset(t *testing.T, root, path, body string) string {
	t.Helper()
	name := filepath.Join(root, path)
	if err := os.MkdirAll(filepath.Dir(name), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(name, []byte(body), 0644); err != nil {
		t.Fatal(err)
	}
	return name
}

func assetProject(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	putAsset(t, root, "mind.yaml", "schema: '1'\n")
	putAsset(t, root, "mind-index.yaml", "schema: '1'\nterms: [] # retain exact bytes\n")
	return root
}

func fixedService() Service {
	return Service{Now: func() time.Time { return time.Date(2026, 1, 2, 3, 4, 5, 0, time.UTC) }}
}

func registerAssets(t *testing.T, root string, items []model.Asset) {
	t.Helper()
	document, err := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	if err != nil {
		t.Fatal(err)
	}
	value := document.Value
	value.Assets = &items
	raw, err := document.Render(value)
	if err != nil {
		t.Fatal(err)
	}
	putAsset(t, root, "mind-index.yaml", string(raw))
}

func TestUpdateAllPreservesRegistrationAndAuthorSections(t *testing.T) {
	root := assetProject(t)
	putAsset(t, root, "assets/chart.png", "new bytes\x00\xff")
	items := []model.Asset{{Name: "custom", Path: "assets/chart.png", Kind: "image", Size: 1, Hash: "old", AddedAt: "original", Tags: []string{"visual"}}, {Name: "missing", Kind: "other", Path: "assets/absent", Size: 7, Hash: "missing-old"}}
	registerAssets(t, root, items)
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	results, err := fixedService().UpdateAll(root, true)
	if err != nil || len(results) != 2 || results[0].Error == nil || !results[1].Changed {
		t.Fatalf("%#v %v", results, err)
	}
	preview, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Equal(before, preview) {
		t.Fatal("preview wrote index")
	}
	results, err = fixedService().UpdateAll(root, false)
	if err != nil {
		t.Fatal(err)
	}
	document, _ := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	got := (*document.Value.Assets)[0]
	if got.Name != "custom" || got.AddedAt != "original" || len(got.Tags) != 1 || got.Hash != results[1].NewHash {
		t.Fatalf("%#v", got)
	}
	raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Contains(raw, []byte("terms: [] # retain exact bytes\n")) {
		t.Fatal("unrelated section changed")
	}
	again, err := fixedService().UpdateOne(root, root, "assets/chart.png", false)
	if err != nil || again.Changed {
		t.Fatalf("%#v %v", again, err)
	}
}

func TestCleanRetainsOrderAndPreviewBytes(t *testing.T) {
	root := assetProject(t)
	putAsset(t, root, "assets/keep.png", "keep")
	registerAssets(t, root, []model.Asset{{Name: "gone", Kind: "other", Path: "assets/missing"}, {Name: "keep", Kind: "image", Path: "assets/keep.png"}})
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	preview, err := fixedService().Clean(root, true)
	if err != nil || preview.RemovedCount != 1 {
		t.Fatalf("%#v %v", preview, err)
	}
	raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Equal(raw, before) {
		t.Fatal("clean preview wrote")
	}
	report, err := fixedService().Clean(root, false)
	if err != nil || report.RemovedCount != 1 {
		t.Fatalf("%#v %v", report, err)
	}
	doc, _ := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	if len(*doc.Value.Assets) != 1 || (*doc.Value.Assets)[0].Name != "keep" {
		t.Fatal("wrong survivor")
	}
}

func TestReconcileRefreshUsesOneCommitAndDoesNotResurrectMissingRows(t *testing.T) {
	root := assetProject(t)
	putAsset(t, root, "assets/chart.png", "current bytes")
	putAsset(t, root, "assets/.hidden/inside.PNG", "inside")
	putAsset(t, root, "assets/.ignored.png", "hidden")
	if err := os.Symlink(".", filepath.Join(root, "assets/loop")); err != nil {
		t.Fatal(err)
	}
	registerAssets(t, root, []model.Asset{{Name: "custom", Kind: "image", Path: "assets/chart.png", Hash: "old", Size: 1, AddedAt: "original", Tags: []string{"visual"}}, {Name: "gone", Kind: "other", Path: "assets/gone", Hash: "missing-old"}})
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	service := fixedService()
	writes := 0
	service.WriteIndex = func(name string, raw []byte) error { writes++; return (store.AtomicWriter{}).Write(name, raw, 0644) }
	preview, err := service.Reconcile(root, true, true)
	if err != nil || preview.KeptCount != 1 || len(preview.Added) != 1 || len(preview.Removed) != 1 || preview.Refreshed == nil || len(*preview.Refreshed) != 2 {
		t.Fatalf("%#v %v", preview, err)
	}
	raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if writes != 0 || !bytes.Equal(before, raw) {
		t.Fatal("refresh preview wrote")
	}
	report, err := service.Reconcile(root, false, true)
	if err != nil || writes != 1 {
		t.Fatalf("%#v %v writes%d", report, err, writes)
	}
	got, err := Show(root, "assets/chart.png")
	if err != nil || got.Hash == "old" || got.Name != "custom" || got.AddedAt != "original" || len(got.Tags) != 1 {
		t.Fatalf("%#v %v", got, err)
	}
	doc, _ := readIndex(root)
	if len(*doc.Value.Assets) != 2 {
		t.Fatal("missing resurrected or hidden file indexed")
	}
	settled, err := service.UpdateOne(root, root, "assets/chart.png", false)
	if err != nil || settled.Changed || writes != 1 {
		t.Fatalf("%#v %v", settled, err)
	}
}

func TestReconcileDoesNotDropAssetsWhenNestedScanFails(t *testing.T) {
	root := assetProject(t)
	path := putAsset(t, root, "assets/private/chart.png", "image bytes")
	registerAssets(t, root, []model.Asset{{Name: "chart", Kind: "image", Path: "assets/private/chart.png"}})
	indexPath := filepath.Join(root, "mind-index.yaml")
	before, err := os.ReadFile(indexPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(filepath.Dir(path), 0); err != nil {
		t.Fatal(err)
	}
	defer func() { _ = os.Chmod(filepath.Dir(path), 0o755) }()
	if _, err := os.ReadDir(filepath.Dir(path)); err == nil {
		t.Skip("filesystem permits reading mode-000 directory")
	}
	if _, err := fixedService().Reconcile(root, false, false); err == nil {
		t.Fatal("asset index accepted an incomplete nested scan")
	}
	after, err := os.ReadFile(indexPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("scan error removed an asset index entry")
	}
}

func TestPublicationPreviewPreservesBytesAndRealUpdateAppendsRecord(t *testing.T) {
	root := assetProject(t)
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	service := fixedService()
	if err := service.SetPublishURL(root, "https://example.invalid/static", "preview", true); err != nil {
		t.Fatal(err)
	}
	raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Equal(before, raw) {
		t.Fatal("publication preview wrote")
	}
	if err := service.SetPublishURL(root, "https://example.invalid/static", "preview", false); err != nil {
		t.Fatal(err)
	}
	doc, _ := readIndex(root)
	if doc.Value.PublishRecords == nil || len(*doc.Value.PublishRecords) != 1 || (*doc.Value.PublishRecords)[0].Path != "_publish_settings/preview" {
		t.Fatal("missing publication record")
	}
}

func TestImportForceLinkAndRollbackRetainEvidence(t *testing.T) {
	root := assetProject(t)
	input := putAsset(t, t.TempDir(), "chart.png", "first bytes")
	service := fixedService()
	name := "custom"
	item, err := service.Add(root, root, input, AddOptions{Name: &name, Tags: []string{"visual"}})
	if err != nil || item.Name != name {
		t.Fatalf("%#v %v", item, err)
	}
	input = putAsset(t, t.TempDir(), "chart.png", "replacement bytes")
	if _, err := service.Add(root, root, input, AddOptions{}); err == nil {
		t.Fatal("overwrote without force")
	}
	failing := service
	failing.WriteIndex = func(string, []byte) error { return os.ErrPermission }
	if _, err := failing.Add(root, root, input, AddOptions{Force: true}); err == nil {
		t.Fatal("ignored index fault")
	}
	raw, _ := os.ReadFile(filepath.Join(root, item.Path))
	if string(raw) != "first bytes" {
		t.Fatal("lost original bytes on failed force")
	}
	item, err = service.Add(root, root, input, AddOptions{Force: true, Link: true})
	if err != nil || item.Name != "chart.png" || len(item.Tags) != 0 {
		t.Fatalf("%#v %v", item, err)
	}
	if info, err := os.Lstat(filepath.Join(root, item.Path)); err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("%v %v", info, err)
	}
	if _, err := service.Add(root, root, filepath.Join(root, item.Path), AddOptions{}); err != nil {
		// An external-target link is treated as the external source and collides at the destination.
		if failure, ok := err.(*output.Error); !ok || failure.Kind != "file_exists" {
			t.Fatal(err)
		}
	}
}

func TestRenameMoveAndRemoveRollbackAtIndexBoundaries(t *testing.T) {
	root, target := assetProject(t), assetProject(t)
	input := putAsset(t, t.TempDir(), "chart.png", "author bytes")
	service := fixedService()
	item, err := service.Add(root, root, input, AddOptions{})
	if err != nil {
		t.Fatal(err)
	}
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	putAsset(t, root, "assets/occupied.png", "occupied")
	failing := service
	failing.WriteIndex = func(string, []byte) error { return os.ErrPermission }
	if err := failing.Rename(root, item.Path, "assets/occupied.png", true); err == nil {
		t.Fatal("ignored rename index fault")
	}
	for path, want := range map[string]string{item.Path: "author bytes", "assets/occupied.png": "occupied"} {
		raw, _ := os.ReadFile(filepath.Join(root, path))
		if string(raw) != want {
			t.Fatal("rename lost bytes")
		}
	}
	if _, err := failing.Remove(root, item.Path, false, false); err == nil {
		t.Fatal("ignored remove index fault")
	}
	if _, err := os.Stat(filepath.Join(root, item.Path)); err != nil {
		t.Fatal("remove did not restore file")
	}
	failing.WriteIndex = func(name string, raw []byte) error {
		if name == filepath.Join(target, "mind-index.yaml") {
			return os.ErrPermission
		}
		return (store.AtomicWriter{}).Write(name, raw, 0644)
	}
	if _, err := failing.Move(root, target, item.Path, false); err == nil {
		t.Fatal("ignored destination fault")
	}
	raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Equal(before, raw) {
		t.Fatal("move did not restore source index")
	}
	if _, err := os.Stat(filepath.Join(root, item.Path)); err != nil {
		t.Fatal("move did not restore file")
	}
	preview, err := service.Move(root, target, item.Path, true)
	if err != nil || !preview.DryRun {
		t.Fatalf("%#v %v", preview, err)
	}
	moved, err := service.Move(root, target, item.Path, false)
	if err != nil {
		t.Fatal(err)
	}
	if err := service.Rename(target, moved.NewPath, "assets/renamed.png", false); err != nil {
		t.Fatal(err)
	}
	got, err := Show(target, "renamed")
	if err != nil || got.AddedAt != item.AddedAt || got.Hash != item.Hash {
		t.Fatalf("%#v %v", got, err)
	}
	report, err := service.Remove(target, got.Path, false, true)
	if err != nil || report.Removed != filepath.Join(target, got.Path) {
		t.Fatalf("%#v %v", report, err)
	}
	if _, err := service.Remove(target, got.Path, false, false); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(target, got.Path)); !os.IsNotExist(err) {
		t.Fatal("file retained")
	}
}

func TestRemoveOnlyMentionsBlockAndForceLeavesExternalLinkTarget(t *testing.T) {
	root := assetProject(t)
	input := putAsset(t, t.TempDir(), "chart.png", "external evidence")
	service := fixedService()
	item, err := service.Add(root, root, input, AddOptions{Link: true})
	if err != nil {
		t.Fatal(err)
	}
	putAsset(t, root, "docs/article.md", "![x](assets/chart.png)")
	document, _ := readIndex(root)
	value := document.Value
	articles := []model.Article{{Title: "Article", Type: "blog", Status: "draft", Path: "docs/article.md"}}
	value.Articles = &articles
	if _, err := service.save(root, document, value); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Remove(root, item.Path, false, false); err == nil {
		t.Fatal("ignored mention")
	}
	if _, err := service.Remove(root, item.Path, true, false); err != nil {
		t.Fatal(err)
	}
	raw, _ := os.ReadFile(input)
	if string(raw) != "external evidence" {
		t.Fatal("deleted external linked evidence")
	}
}
