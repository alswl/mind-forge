package main

import (
	"reflect"
	"testing"
)

func TestCommandSpecFrozenTreeAndAliases(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	if len(spec.Commands) != 131 {
		t.Fatalf("command inventory changed: %d", len(spec.Commands))
	}
	for _, item := range []struct{ input, want []string }{
		{[]string{"terms", "ls"}, []string{"term", "list"}},
		{[]string{"article", "block", "remove"}, []string{"article", "block", "rm"}},
		{[]string{"project", "rm"}, []string{"project", "remove"}},
	} {
		command, ok := spec.Lookup(item.input)
		if !ok || !reflect.DeepEqual(command.Path, item.want) {
			t.Fatalf("alias resolution: %#v => %#v (%t)", item.input, command, ok)
		}
	}
	if _, ok := spec.Lookup([]string{"help"}); ok {
		t.Fatal("help subcommand must remain disabled")
	}
}

func TestAccessClassificationRecoveryOverridesDryRunAndUnknownDefaultsWrite(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range []struct {
		path []string
		mods Modifiers
		want Access
	}{
		{nil, Modifiers{}, Read},
		{[]string{"project", "index"}, Modifiers{}, Write},
		{[]string{"project", "lint"}, Modifiers{}, Read},
		{[]string{"project", "lint"}, Modifiers{Fix: true}, Write},
		{[]string{"project", "lint"}, Modifiers{Fix: true, DryRun: true}, Read},
		{[]string{"mind", "index"}, Modifiers{Check: true}, Read},
		{[]string{"additional", "rename"}, Modifiers{}, Recovery},
		{[]string{"additional", "remove"}, Modifiers{}, Recovery},
		{[]string{"additional", "rename"}, Modifiers{DryRun: true}, Recovery},
		{[]string{"additional", "remove"}, Modifiers{DryRun: true}, Recovery},
		{[]string{"lifecycle", "apply"}, Modifiers{DryRun: true}, Recovery},
		{[]string{"future", "list"}, Modifiers{DryRun: true}, Write},
	} {
		if got := spec.Classify(item.path, item.mods); got != item.want {
			t.Fatalf("%v %#v access=%s want %s", item.path, item.mods, got, item.want)
		}
	}
	for _, item := range []struct {
		path     []string
		required bool
	}{
		{[]string{"project", "index"}, false}, {[]string{"project", "list"}, true}, {[]string{"config", "generate"}, false}, {[]string{"version"}, false}, {[]string{"future"}, true},
	} {
		if got := spec.RequiresRepo(item.path); got != item.required {
			t.Fatalf("repo requirement %v: %t", item.path, got)
		}
	}
}

func TestFlagSpecScopesAliasesRepeatAndConstraints(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range []struct {
		path                 []string
		spelling, name, kind string
		repeat               bool
	}{
		{[]string{"source", "new"}, "-n", "dry-run", "bool", false},
		{[]string{"source", "new"}, "--name", "name", "string", false},
		{[]string{"article", "new"}, "--single-file", "file", "bool", false},
		{[]string{"config", "generate"}, "-o", "output", "string", false},
		{[]string{"config", "generate"}, "--out", "out", "string", false},
		{[]string{"term", "lint"}, "--term", "term", "string", true},
		{[]string{"term", "lint"}, "--max-warnings", "max-warnings", "int", false},
	} {
		flag, ok := spec.Flag(item.path, item.spelling)
		if !ok || flag.Name != item.name || flag.Kind != item.kind || flag.Repeat != item.repeat {
			t.Fatalf("flag %v %s => %#v (%t)", item.path, item.spelling, flag, ok)
		}
	}
	flag, _ := spec.Flag([]string{"additional", "remove"}, "--emit-plan")
	if !reflect.DeepEqual(flag.Requires, []string{"dry-run"}) {
		t.Fatal(flag)
	}
	flag, _ = spec.Flag([]string{"lifecycle", "convert"}, "--to")
	if !flag.Required || !reflect.DeepEqual(flag.Choices, []string{"category", "bundle"}) {
		t.Fatal(flag)
	}
	flag, _ = spec.Flag(nil, "--generate-manual")
	if !flag.Hidden {
		t.Fatal("manual flag must be hidden")
	}
}
