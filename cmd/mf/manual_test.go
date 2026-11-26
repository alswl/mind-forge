package main

import (
	"strings"
	"testing"

	"github.com/spf13/pflag"
)

func TestManualRendersCommandContract(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	text := Manual(spec)
	if !strings.HasSuffix(text, "\n") || strings.HasSuffix(text, "\n\n") {
		t.Error("manual must end with exactly one newline")
	}
	for _, want := range []string{
		"## `mf config generate`", "**Usage:** `mf config generate [OPTIONS] --out <OUTPUT>`",
		"* `--out <OUTPUT>`", "* `-o`, `--output <FORMAT>` — Output format",
		"Possible values: `bash`, `zsh`, `fish`, `powershell`, `elvish`",
		"* `<PATH>` — Target directory path (defaults to current directory)",
		"**Command Alias:** `ls`",
		"* `--file` [alias: `single-file`] — Write a single file instead of a directory",
	} {
		if !strings.Contains(text, want) {
			t.Errorf("manual is missing %q", want)
		}
	}
	if strings.Contains(text, "* `--generate-manual`") || strings.Contains(text, "clap-markdown") {
		t.Fatal("manual exposes a hidden flag or claims a retired generator")
	}
	for _, wrong := range []string{"* `<IDENTITY>` — <NAME>", "* `<TERM>` — <ORIGINAL>", "— Usage: mf", "\nUsage: mf mind show"} {
		if strings.Contains(text, wrong) {
			t.Errorf("manual invents a description from command syntax: %q", wrong)
		}
	}
}

func TestArticleNewBooleanDefaultsMatchPublishedContract(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for name, want := range map[string]bool{"draft": true, "file": false} {
		flag, ok := spec.Flag([]string{"article", "new"}, "--"+name)
		if !ok {
			t.Fatalf("missing flag %s", name)
		}
		flags := pflag.NewFlagSet("article", pflag.ContinueOnError)
		if err := registerFlag(flags, flag); err != nil {
			t.Fatal(err)
		}
		if err := flags.Parse(nil); err != nil {
			t.Fatal(err)
		}
		if got, err := flags.GetBool(name); err != nil || got != want {
			t.Errorf("%s default=%v, %v; want %v", name, got, err, want)
		}
		if flag.Default == nil || *flag.Default != flags.Lookup(name).DefValue {
			t.Errorf("%s default is omitted from the generated contract", name)
		}
	}
}

func TestManualArgumentDescriptionStaysOnItsArgument(t *testing.T) {
	item := CommandSpec{Help: "Arguments:\n  <FIRST>  \n  <SECOND>  Second description\n            continued here\n\nOptions:\n"}
	if got := manualArgumentHelp(item, "FIRST"); got != "" {
		t.Fatalf("empty argument borrowed next argument's description: %q", got)
	}
	if got := manualArgumentHelp(item, "SECOND"); got != "Second description continued here" {
		t.Fatalf("wrapped description=%q", got)
	}
}

func TestArticleStatusAppearsInHelpAndManual(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	manual := Manual(spec)
	for _, action := range []string{"list", "lint", "update"} {
		item, ok := spec.Lookup([]string{"article", action})
		if !ok {
			t.Fatalf("missing article %s", action)
		}
		_, section, _ := strings.Cut(manual, "## `mf article "+action+"`")
		section, _, _ = strings.Cut(section, "\n## ")
		for _, text := range []string{item.Help, item.HelpDefault, section} {
			if !strings.Contains(text, "--status <STATUS>") || !strings.Contains(text, "draft, published, completed, abandoned") {
				t.Errorf("article %s help or manual is missing the status contract", action)
			}
		}
	}
}
