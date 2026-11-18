package build

import (
	"reflect"
	"strings"
	"testing"
)

func TestPrivateCalloutFiltering(t *testing.T) {
	for _, marker := range []string{"mf-private", "mind-forge-private", "MF-Private", "mf-draft"} {
		input := "Before.\n\n> [!" + marker + "] title\n> Secret.\n\nAfter.\n"
		got, warnings := StripPrivateCallouts(input)
		if got != "Before.\n\nAfter.\n" || len(warnings) != 0 {
			t.Fatalf("%s: %q %+v", marker, got, warnings)
		}
	}
	for _, gap := range []string{"\n", "\n\n", " \n\t\n"} {
		input := "Before.\n\n> [!mf-private]\n> first\n" + gap + "> ordinary quote\n\nAfter.\n"
		got, warnings := StripPrivateCallouts(input)
		if got != "Before.\n\nAfter.\n" || !reflect.DeepEqual(warnings, []PrivateWarning{{StartLine: 3}}) {
			t.Fatalf("gap %q: %q %+v", gap, got, warnings)
		}
	}
}

func TestPrivateCalloutLiteralAndSpacing(t *testing.T) {
	for _, input := range []string{
		"Before.\n```\n> [!mf-private]\n> literal\n```\nAfter.\n",
		"~~~\n> [!mf-private]\n> literal\n~~~\n",
		"Before.\n\n> [!note]\n> Public.\n",
		"    > [!mf-private]\n    > Indented code.\n",
		"One.\n\n\nTwo.\n\n\n",
	} {
		got, warnings := StripPrivateCallouts(input)
		if got != input || len(warnings) != 0 {
			t.Fatalf("literal changed: %q -> %q %+v", input, got, warnings)
		}
	}
	for _, c := range []struct{ input, want string }{
		{"Before.\n\n> [!mf-private]\n> Secret.\n\n", "Before.\n\n"},
		{"## Section\n\n> [!mf-private]\n> Secret.\n\nText.\n", "## Section\n\nText.\n"},
		{"> [!mf-private]\n> Secret.\n\nPublic.\n", "Public.\n"},
		{"Before.\n\n> [!mf-private]\n> Secret.\n\n```\n> code\n```\n", "Before.\n\n```\n> code\n```\n"},
	} {
		got, _ := StripPrivateCallouts(c.input)
		if got != c.want {
			t.Fatalf("got %q want %q", got, c.want)
		}
		again, _ := StripPrivateCallouts(got)
		if again != got {
			t.Fatal("not idempotent")
		}
	}
}

func TestAssembleBlocksPrivacyAndMetadata(t *testing.T) {
	blocks := []Block{
		{Path: "title.md", Content: "---\ntitle: Public\ntypora-copy-images-to: assets\nmind-forge-visibility: public\n---\n# Public\nIntro.\n"},
		{Path: "private.md", Content: "---\nmind-forge-visibility: private\n---\nSecret block.\n"},
		{Path: "body.md", Content: "## Body\n\n> [!mf-private]\n> Secret start.\n\n> Secret end.\n\nPublic body."},
	}
	got, warnings, err := AssembleBlocks(blocks, false, "")
	if err != nil {
		t.Fatal(err)
	}
	want := "---\ntitle: Public\n---\n# Public\nIntro.\n## Body\n\nPublic body.\n"
	if got != want {
		t.Fatalf("got %q want %q", got, want)
	}
	if len(warnings) != 1 || !strings.HasPrefix(warnings[0], "body.md:3:") {
		t.Fatalf("warning provenance: %q", warnings)
	}
	if blocks[0].Content == got || !strings.Contains(blocks[1].Content, "Secret block") {
		t.Fatal("input mutated")
	}
}

func TestAssembleBlocksRejectsPrivateTitleAndInvalidVisibility(t *testing.T) {
	for _, blocks := range [][]Block{
		{{Path: "first.md", Content: "---\nmind-forge-visibility: private\n---\n# Title\n"}},
		{{Path: "first.md", Content: "# Title\n"}, {Path: "bad.md", Content: "---\nmind-forge-visibility: secret\n---\nText\n"}},
	} {
		got, _, err := AssembleBlocks(blocks, false, "")
		if err == nil || got != "" {
			t.Fatalf("should reject without partial output: %q %v", got, err)
		}
	}
}

func TestAssembleTitleAndBanner(t *testing.T) {
	got, _, err := AssembleBlocks([]Block{{Content: "---\ntitle: Public\n---\n\n# Public\n\nBody.\n# Later\n"}}, true, "Notice.\n\n")
	if err != nil {
		t.Fatal(err)
	}
	want := "---\ntitle: Public\n---\n\nNotice.\n\n\n\nBody.\n# Later\n"
	if got != want {
		t.Fatalf("got %q want %q", got, want)
	}
}
