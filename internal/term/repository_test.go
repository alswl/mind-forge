package term

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestTermPersistenceAndAppendPreserveAuthority(t *testing.T) {
	for _, kind := range []string{"project", "global"} {
		t.Run(kind, func(t *testing.T) {
			root := t.TempDir()
			scope := Scope{Root: root, Kind: kind}
			original := []byte("schema: '1'\n# preserved\npublish_records: []\nunknown: keep\n")
			if err := os.WriteFile(scope.Path(), original, 0644); err != nil {
				t.Fatal(err)
			}
			first, err := scope.New("API", Input{Aliases: []string{"api", "api"}, Tags: []string{"core"}, Definition: stringPtr("first")}, []string{"ap1:API"}, false)
			if err != nil || !first.Created || len(first.Term.Corrections) != 2 {
				t.Fatalf("new: %+v %v", first, err)
			}
			second, err := scope.New("API", Input{Definition: stringPtr("ignored"), Aliases: []string{"api", "Api"}, Tags: []string{"core", "extra"}}, []string{"ap2:API"}, false)
			if err != nil || second.Created || *second.Term.Definition != "first" || len(second.AddedAliases) != 1 || len(second.Term.Corrections) != 4 {
				t.Fatalf("append: %+v %v", second, err)
			}
			raw, err := os.ReadFile(scope.Path())
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Contains(raw, []byte("# preserved\npublish_records: []\nunknown: keep\n")) {
				t.Fatalf("unrelated sections changed: %s", raw)
			}
			before := append([]byte{}, raw...)
			if _, err := scope.New("Other", Input{Aliases: []string{"api"}}, nil, false); err == nil {
				t.Fatal("alias collision accepted")
			}
			raw, _ = os.ReadFile(scope.Path())
			if !bytes.Equal(before, raw) {
				t.Fatal("rejected mutation wrote bytes")
			}
			if _, err := scope.New("Preview", Input{}, nil, true); err != nil {
				t.Fatal(err)
			}
			raw, _ = os.ReadFile(scope.Path())
			if !bytes.Equal(before, raw) {
				t.Fatal("dry-run wrote bytes")
			}
		})
	}
}

func TestLenientCorrectionRepair(t *testing.T) {
	for _, kind := range []string{"project", "global"} {
		scope := Scope{Root: t.TempDir(), Kind: kind}
		raw := []byte("schema: '1'\nterms:\n- term: API\n  corrections:\n  - original: api_\n    correct: API\n")
		if err := os.WriteFile(scope.Path(), raw, 0644); err != nil {
			t.Fatal(err)
		}
		if _, err := scope.Load(true); err == nil {
			t.Fatal("strict load accepted invalid standalone edges")
		}
		if _, err := scope.Show("API"); err != nil {
			t.Fatalf("invalid term not inspectable: %v", err)
		}
		boundary := "loose"
		if _, err := scope.UpdateCorrection("API", "api_", model.CorrectionPatch{Boundary: &boundary}, false); err != nil {
			t.Fatal(err)
		}
		terms, err := scope.Load(true)
		if err != nil || terms[0].Corrections[0].Boundary != "loose" {
			t.Fatalf("repair: %+v %v", terms, err)
		}
	}
}

func TestCorrectionIdempotenceShadowWarningAndRejectedRepair(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "project"}
	if _, err := scope.New("API", Input{}, nil, false); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.New("Other", Input{}, nil, false); err != nil {
		t.Fatal(err)
	}
	c := model.Correction{Original: "api", Correct: "Other"}
	_, created, warning, err := scope.AddCorrection("Other", c, false)
	if err != nil || !created || warning == "" {
		t.Fatalf("shadowing should warn and proceed: %v %v %s", err, created, warning)
	}
	_, created, warning, err = scope.AddCorrection("Other", c, false)
	if err != nil || created || warning != "" {
		t.Fatalf("same pair not idempotent: %v %v %s", err, created, warning)
	}
	before, _ := os.ReadFile(scope.Path())
	bad := "api_"
	if _, err = scope.UpdateCorrection("Other", "api", model.CorrectionPatch{Correct: &bad}, true); err != nil {
		t.Fatal(err)
	}
	after, _ := os.ReadFile(scope.Path())
	if !bytes.Equal(before, after) {
		t.Fatal("correction dry-run wrote")
	}
	if _, err = scope.RemoveCorrection("Other", "missing", false); err == nil {
		t.Fatal("missing correction silently ignored")
	}
}

func TestTermUpdateCorrectionAttributesAndMetadataClear(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	confidence := 0.7
	if _, err := scope.New("API", Input{Description: stringPtr("description"), Confidence: &confidence, Aliases: []string{"old"}}, nil, false); err != nil {
		t.Fatal(err)
	}
	tm, err := scope.Update("API", Patch{ClearDescription: true, ClearConfidence: true, DeleteAliases: []string{"old"}, AddCorrections: []string{"ap1", "ap2:ALT"}, CorrectionMatches: []string{"ap1:pinyin"}, CorrectionPinyins: []string{"ap1:a-p-i"}}, false)
	if err != nil {
		t.Fatal(err)
	}
	if tm.Description != nil || tm.Confidence != nil || len(tm.Aliases) != 0 || len(tm.Corrections) != 3 || tm.Corrections[1].Correct != "API" || tm.Corrections[1].Match != "pinyin" || tm.Corrections[1].Boundary != "loose" {
		t.Fatalf("update: %+v", tm)
	}
	if _, err = scope.Update("API", Patch{}, false); err == nil {
		t.Fatal("empty update accepted")
	}
	if _, err = scope.Update("API", Patch{Description: stringPtr("x"), ClearDescription: true}, false); err == nil {
		t.Fatal("conflicting metadata accepted")
	}
}

func TestGlobalSchemaAndMissingScope(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	terms, err := scope.Load(true)
	if err != nil || len(terms) != 0 {
		t.Fatalf("missing global: %v %v", terms, err)
	}
	for _, raw := range []string{"terms: []\n", "schema: '2'\nterms: []\n", "schema: '1'\nterms: null\n", "schema: '1'\nterms: [null]\n", "schema: '1'\nschema_version: '1'\n"} {
		if err := os.WriteFile(filepath.Join(scope.Root, "minds-terms.yaml"), []byte(raw), 0644); err != nil {
			t.Fatal(err)
		}
		if _, err := scope.Load(true); err == nil {
			t.Fatalf("accepted malformed global: %q", raw)
		}
	}
}

func TestGlobalTermEditPreservesAliasesToRemovedAnchors(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	raw := []byte("schema_version: '1'\nterms: &catalogue\n- term: API\nunknown: *catalogue\n")
	if err := os.WriteFile(scope.Path(), raw, 0644); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.New("Other", Input{}, nil, false); err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(scope.Path())
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(got, []byte("*catalogue")) {
		t.Fatalf("dangling anchor left behind: %s", got)
	}
	if !bytes.Contains(got, []byte("unknown:")) {
		t.Fatal("unknown authority dropped")
	}
	if _, err := scope.Load(true); err != nil {
		t.Fatal(err)
	}
}

func TestNewRetainsReferenceRawMisrecognitionAndAllowsRepair(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	if _, err := scope.New("API", Input{}, []string{"api_:API"}, false); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.Load(true); err == nil {
		t.Fatal("strict read ignored standalone invariant")
	}
	if _, err := scope.Show("API"); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.RemoveCorrection("API", "api_", false); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.Load(true); err != nil {
		t.Fatal(err)
	}
}
