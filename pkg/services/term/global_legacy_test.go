package term

import (
	"os"
	"reflect"
	"strings"
	"testing"
)

func legacyGlobalScope(t *testing.T) Scope {
	t.Helper()
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	if err := os.WriteFile(scope.Path(), []byte("schema_version: '1'\nterms: []\n"), 0600); err != nil {
		t.Fatal(err)
	}
	return scope
}

func TestLegacyGlobalTermRepository(t *testing.T) {
	t.Run("new_and_list", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		got, err := scope.New("api-gateway", Input{Definition: stringPtr("The API gateway service")}, nil, false)
		if err != nil {
			t.Fatal(err)
		}
		if got.Term.Term != "api-gateway" || got.Term.Definition == nil || *got.Term.Definition != "The API gateway service" {
			t.Fatalf("new = %+v", got)
		}
		list, err := List(scope.Root, "", ListFilter{})
		if err != nil || len(list) != 1 || list[0].Term.Term != "api-gateway" {
			t.Fatalf("list = %+v, %v", list, err)
		}
	})
	t.Run("duplicate_term_is_idempotent", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		if _, err := scope.New("foo", Input{}, nil, false); err != nil {
			t.Fatal(err)
		}
		got, err := scope.New("foo", Input{}, nil, false)
		if err != nil || got.Created {
			t.Fatalf("duplicate = %+v, %v", got, err)
		}
	})
	t.Run("empty_term_name_rejected", func(t *testing.T) {
		_, err := legacyGlobalScope(t).New("", Input{}, nil, false)
		if err == nil || !strings.Contains(err.Error(), "cannot be empty") {
			t.Fatalf("empty term = %v", err)
		}
	})
	t.Run("show_term_found", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		if _, err := scope.New("rust", Input{Definition: stringPtr("A systems programming language"), Tags: []string{"lang"}}, nil, false); err != nil {
			t.Fatal(err)
		}
		got, err := scope.Show("rust")
		if err != nil || got.Term != "rust" || !reflect.DeepEqual([]string(got.Tags), []string{"lang"}) {
			t.Fatalf("show = %+v, %v", got, err)
		}
	})
	t.Run("show_term_not_found", func(t *testing.T) {
		_, err := (Scope{Root: t.TempDir(), Kind: "global"}).Show("nonexistent")
		if err == nil || !strings.Contains(err.Error(), "not found") {
			t.Fatalf("missing term = %v", err)
		}
	})
	t.Run("fix_term_definition", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		if _, err := scope.New("k8s", Input{Definition: stringPtr("Kubernetes")}, nil, false); err != nil {
			t.Fatal(err)
		}
		got, err := scope.Update("k8s", Patch{Definition: stringPtr("Kubernetes (K8s)")}, false)
		if err != nil || got.Definition == nil || *got.Definition != "Kubernetes (K8s)" {
			t.Fatalf("definition = %+v, %v", got, err)
		}
	})
	t.Run("filter_terms", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		for name, tag := range map[string]string{"alpha": "test", "beta": "prod", "gamma": "test"} {
			if _, err := scope.New(name, Input{Tags: []string{tag}}, nil, false); err != nil {
				t.Fatal(err)
			}
		}
		got, err := List(scope.Root, "", ListFilter{Tags: []string{"test"}})
		if err != nil || len(got) != 2 {
			t.Fatalf("filtered = %+v, %v", got, err)
		}
	})
	t.Run("alias_conflict_rejected", func(t *testing.T) {
		scope := legacyGlobalScope(t)
		if _, err := scope.New("alpha", Input{Aliases: []string{"a"}}, nil, false); err != nil {
			t.Fatal(err)
		}
		_, err := scope.New("beta", Input{Aliases: []string{"a"}}, nil, false)
		if err == nil || !strings.Contains(err.Error(), "conflicts") {
			t.Fatalf("alias conflict = %v", err)
		}
	})
	t.Run("loads_empty_when_file_missing", func(t *testing.T) {
		scope := Scope{Root: t.TempDir(), Kind: "global"}
		got, err := scope.Load(true)
		if err != nil || len(got) != 0 {
			t.Fatalf("missing global terms = %+v, %v", got, err)
		}
	})
}
