package main

import (
	"errors"
	"reflect"
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

func TestParserPreservesEarlierUnknownSubcommand(t *testing.T) {
	for _, test := range []struct {
		name, kind, value string
		path, args        []string
	}{
		{"removed-command", "unknown_command", "learn", []string{"term"}, []string{"learn", "--original"}},
		{"noun-alias", "unknown_command", "learn", []string{"terms"}, []string{"learn", "--original"}},
		{"later-invalid-value", "unknown_command", "learn", []string{"term"}, []string{"learn", "--output", "invalid"}},
		{"prior-global-value", "unknown_command", "learn", []string{"term"}, []string{"--project", "lab", "learn", "--original"}},
		{"removed-project-command", "unknown_command", "status", []string{"project"}, []string{"status", "--bogus"}},
		{"nested-command", "unknown_command", "bogus", []string{"config"}, []string{"bogus", "--bogus"}},
		{"earlier-flag", "unexpected_flag", "--original", []string{"term"}, []string{"--original", "learn"}},
		{"flag-value-is-not-command", "unexpected_flag", "--original", []string{"term"}, []string{"--project", "learn", "--original"}},
		{"leaf-positional", "unexpected_flag", "--bogus", []string{"source", "new"}, []string{"synthetic.md", "--bogus"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			command := parserCommand(t, test.path)
			err := command.ParseFlags(test.args)
			if err == nil {
				t.Fatal("expected a flag parser error")
			}
			err = flagParseError(command, err)
			var failure *parseFailure
			if !errors.As(err, &failure) || failure.kind != test.kind || failure.value != test.value {
				t.Fatalf("failure = %v, want %s: %s", err, test.kind, test.value)
			}
		})
	}
}

func parserCommand(t *testing.T, path []string) *cobra.Command {
	t.Helper()
	root, err := NewCommand(nil)
	if err != nil {
		t.Fatal(err)
	}
	command, remaining, err := root.Find(path)
	if err != nil || len(remaining) != 0 {
		t.Fatalf("find %v: %v, %v", path, remaining, err)
	}
	return command
}

func TestCobraCompletionIncludesVisibleAliases(t *testing.T) {
	for _, test := range []struct{ path, aliases []string }{
		{nil, []string{"terms"}},
		{[]string{"project"}, []string{"ls", "rm"}},
		{[]string{"term"}, []string{"ls", "rm", "mv"}},
		{[]string{"article", "block"}, []string{"remove"}},
	} {
		command := parserCommand(t, test.path)
		values, _ := command.ValidArgsFunction(command, nil, "")
		for _, alias := range test.aliases {
			found := false
			for _, value := range values {
				found = found || strings.SplitN(value, "\t", 2)[0] == alias
			}
			if !found {
				t.Errorf("%v completion lacks visible alias %q", test.path, alias)
			}
		}
	}
	command := parserCommand(t, []string{"article", "new"})
	alias := command.Flags().Lookup("single-file")
	if alias == nil || alias.Hidden {
		t.Error("visible --single-file alias is hidden from completion")
	}
}

func TestCobraCompletionOmitsUnsupportedHelpCommand(t *testing.T) {
	for _, completing := range []bool{false, true} {
		root := parserCommand(t, nil)
		root.InitDefaultHelpCmd()
		selected, _, err := root.Find([]string{"version"})
		if err != nil {
			t.Fatal(err)
		}
		if completing {
			selected = &cobra.Command{Use: cobra.ShellCompRequestCmd}
			root.AddCommand(selected)
		}
		if root.PersistentPreRun != nil {
			root.PersistentPreRun(selected, nil)
		}
		found := false
		for _, child := range root.Commands() {
			found = found || child.Name() == "help"
		}
		if found == completing {
			t.Errorf("completion=%v: unsupported help registered=%v", completing, found)
		}
		if root.Flags().Lookup("help") == nil {
			t.Error("supported --help flag removed")
		}
	}
}

func TestCobraCommandRegistration(t *testing.T) {
	command := parserCommand(t, []string{"terms", "ls"})
	if command.CommandPath() != "mf term list" {
		t.Fatal(command.CommandPath())
	}
	if err := command.ParseFlags([]string{"-vv", "--tag", "a,b", "--tag", "second", "--json"}); err != nil {
		t.Fatal(err)
	}
	if values, err := command.Flags().GetStringArray("tag"); err != nil || !reflect.DeepEqual(values, []string{"a,b", "second"}) {
		t.Fatalf("repeated flags must not split commas: %#v (%v)", values, err)
	}
	if count, _ := command.Flags().GetCount("verbose"); count != 2 {
		t.Fatalf("verbosity count = %d", count)
	}
	generate := parserCommand(t, []string{"config", "generate"})
	if err := generate.ParseFlags([]string{"-ojson", "--out", "saved.yaml"}); err != nil {
		t.Fatal(err)
	}
	if got, _ := generate.Flags().GetString("out"); got != "saved.yaml" {
		t.Fatal(got)
	}
	if got, _ := generate.Flags().GetString("output"); got != "json" {
		t.Fatal(got)
	}
}

func TestCobraGlobalChoiceCompletionIsInherited(t *testing.T) {
	for _, path := range [][]string{nil, {"source", "new"}} {
		command := parserCommand(t, path)
		complete, ok := command.GetFlagCompletionFunc("output")
		if !ok {
			t.Errorf("%v lacks completion for inherited --output", path)
			continue
		}
		values, directive := complete(command, nil, "j")
		if !reflect.DeepEqual(values, []string{"text", "json"}) || directive != cobra.ShellCompDirectiveNoFileComp {
			t.Errorf("%v output choices=%v directive=%v", path, values, directive)
		}
	}
}

func TestCobraFlagValidation(t *testing.T) {
	for _, test := range []struct{ path, flags, args []string }{
		{[]string{"config", "generate"}, nil, nil},
		{[]string{"version"}, []string{"--output", "invalid"}, nil},
		{[]string{"version"}, []string{"--quiet", "--verbose"}, nil},
		{[]string{"article", "convert"}, []string{"--to-single-file", "--to-directory"}, nil},
		{[]string{"additional", "remove"}, []string{"--emit-plan"}, nil},
		{[]string{"version"}, []string{"--root", "a", "--root", "b"}, nil},
		{[]string{"completion"}, nil, []string{"unknown"}},
		{[]string{"version"}, nil, []string{"extra"}},
	} {
		command := parserCommand(t, test.path)
		err := command.ParseFlags(test.flags)
		if err == nil {
			err = validateCommand(command, test.args)
		}
		if err == nil {
			t.Errorf("accepted invalid invocation %v %v %v", test.path, test.flags, test.args)
		}
	}
}
