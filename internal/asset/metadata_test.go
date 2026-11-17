package asset

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestMetadataHashesRawBytesAndMeasuresSize(t *testing.T) {
	name := filepath.Join(t.TempDir(), "sample.bin")
	if err := os.WriteFile(name, []byte("abc"), 0600); err != nil {
		t.Fatal(err)
	}
	size, hash, err := Metadata(name)
	if err != nil || size != 3 || hash != "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad" {
		t.Fatalf("size%d hash%s err%v", size, hash, err)
	}
	if _, _, err := Metadata(filepath.Dir(name)); err == nil {
		t.Fatal("directory accepted as asset bytes")
	}
}

func TestAssetKindAndNamesRetainExtensionRules(t *testing.T) {
	for _, test := range []struct{ path, kind string }{{"a.PNG", "image"}, {"a.jpeg", "image"}, {"a.svg", "image"}, {"a.MOV", "video"}, {"a.webm", "video"}, {"a.m4a", "audio"}, {"a.FLAC", "audio"}, {"a.ogg", "audio"}, {"a.PDF", "other"}, {".png", "other"}, {"archive.tar.gz", "other"}, {"机器.png", "image"}} {
		if got := InferKind(test.path); got != test.kind {
			t.Fatalf("%s: %s want%s", test.path, got, test.kind)
		}
	}
	for _, test := range []struct{ path, name string }{{"assets/chart.png", "chart"}, {"assets/archive.tar.gz", "archive.tar"}, {"assets/.hidden", ".hidden"}, {"assets/new.", "new"}} {
		if got := RenameName(test.path); got != test.name {
			t.Fatalf("%s name%s want%s", test.path, got, test.name)
		}
	}
}

func TestAssetReferenceScanDistinguishesIndexRowsFromLiteralMentions(t *testing.T) {
	root := t.TempDir()
	for name, body := range map[string]string{"mentioned.md": "![x](assets/chart.png)\n", "unrelated.md": "# Author\n"} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(body), 0600); err != nil {
			t.Fatal(err)
		}
	}
	articles := []model.Article{{Title: "Mentioned", Path: "mentioned.md"}, {Title: "Unrelated", Path: "unrelated.md"}, {Title: "Missing", Path: "missing.md"}}
	index := model.DefaultIndex()
	index.Articles = &articles
	mentioned, refs := References(root, index, "chart.png")
	if !mentioned || len(refs) != 4 || refs[0].Kind != "index" || refs[1].Kind != "mention" || refs[2].FromID != "Unrelated" {
		t.Fatalf("mentioned%v refs%#v", mentioned, refs)
	}
	mentioned, refs = References(root, index, "else.png")
	if mentioned || len(refs) != 3 {
		t.Fatalf("index rows incorrectly block unmentioned asset: %v %#v", mentioned, refs)
	}
}
