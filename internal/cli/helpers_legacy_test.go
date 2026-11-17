package cli

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/spf13/pflag"
)

func TestLegacySharedFlagParsing(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name, flag, arg string
		bad             bool
	}{
		{"dry_run_flag_long_name", "--dry-run", "--dry-run", false},
		{"dry_run_flag_no_short", "--dry-run", "-d", true},
		{"force_flag_long_name", "--force", "--force", false},
		{"force_flag_short_f", "--force", "-f", false},
		{"yes_flag_long_name", "--yes", "--yes", false},
		{"yes_flag_short_y", "--yes", "-y", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			flag, ok := spec.Flag([]string{"source", "remove"}, test.flag)
			if !ok {
				t.Fatalf("shared flag absent: %s", test.flag)
			}
			flags := pflag.NewFlagSet("isolated", pflag.ContinueOnError)
			if err := registerFlag(flags, flag); err != nil {
				t.Fatal(err)
			}
			err := flags.Parse([]string{test.arg})
			if test.bad {
				if err == nil || !strings.Contains(err.Error(), "d") {
					t.Fatalf("unexpected -d result: %v", err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			value, err := flags.GetBool(flag.Name)
			if err != nil || !value {
				t.Fatalf("shared flag not parsed true: %v %v", value, err)
			}
		})
	}
}

func TestLegacyAccessFamilies(t *testing.T) {
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		path []string
		mods Modifiers
		want Access
	}{
		{[]string{"mind", "new"}, Modifiers{}, Write},
		{[]string{"output", "new"}, Modifiers{}, Write},
		{[]string{"project", "index"}, Modifiers{}, Write},
		{[]string{"article", "index"}, Modifiers{}, Write},
		{[]string{"source", "index"}, Modifiers{}, Write},
		{[]string{"source", "clean"}, Modifiers{}, Write},
		{[]string{"asset", "index"}, Modifiers{}, Write},
		{[]string{"term", "new"}, Modifiers{}, Write},
		{[]string{"term", "fix"}, Modifiers{}, Write},
		{[]string{"project", "lint"}, Modifiers{Fix: true}, Write},
		{[]string{"build"}, Modifiers{}, Write},
		{[]string{"publish", "run"}, Modifiers{}, Write},
		{[]string{"config", "generate"}, Modifiers{}, Write},
		{[]string{"mind", "new"}, Modifiers{DryRun: true}, Read},
		{[]string{"project", "lint"}, Modifiers{}, Read},
		{[]string{"source", "list"}, Modifiers{}, Read},
		{[]string{"mind", "list"}, Modifiers{}, Read},
		{[]string{"output", "show"}, Modifiers{}, Read},
	} {
		if _, ok := spec.Lookup(test.path); !ok {
			t.Fatalf("access case references unknown command: %v", test.path)
		}
		if got := spec.Classify(test.path, test.mods); got != test.want {
			t.Fatalf("access %v %+v = %s, want %s", test.path, test.mods, got, test.want)
		}
	}
}

func TestLegacyLintFindingPreview(t *testing.T) {
	count, confidence := uint64(1), 0.9
	r := model.TermLintReport{Findings: []model.TermFinding{{Path: "synthetic.md", Line: 2, Column: 4, Original: "old", Correct: "new", Term: "Synthetic", Confidence: &confidence, ReplacementEligible: true, MatchKind: "word", FixKind: "suggested", Boundary: "standalone", BoundaryMode: "standalone", Selection: "selected", Context: "context with old token"}}, ScannedFiles: 1, WouldFixCount: &count, SelectedCount: 1}
	text := formatTermLint(r, true, true, "file", "")
	for _, part := range []string{"synthetic.md:2:4", `"old" → "new"`, "match=word", "selection=selected", "context with old token", "selected 1"} {
		if !strings.Contains(text, part) {
			t.Errorf("preview missing %q: %s", part, text)
		}
	}
}

func TestLegacyArticleMtime(t *testing.T) {
	for _, test := range []struct {
		name    string
		seconds uint64
		want    string
	}{
		{"format_mtime_zero_returns_dash", 0, "-"},
		{"format_mtime_known_timestamp", 1705307400, "2024-01-15 08:30"},
		{"format_mtime_epoch_boundary", 1, "1970-01-01 00:00"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := articleMtime(test.seconds); got != test.want {
				t.Fatalf("mtime(%d) = %q, want %q", test.seconds, got, test.want)
			}
			if test.name == "format_mtime_epoch_boundary" && articleMtime(0) != "-" {
				t.Fatal("epoch zero must remain unknown")
			}
		})
	}
}
