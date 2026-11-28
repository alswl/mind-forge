package build

import (
	"reflect"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
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
		"~~~markdown\n> [!mf-private] Handoff\n> Decision: literal\n~~~\n",
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
		{"~~~markdown\nLiteral.\n~~~\n\n> [!mf-private] Handoff\n> Decision: Secret.\n\nPublic.\n", "~~~markdown\nLiteral.\n~~~\n\nPublic.\n"},
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
	require.Len(t, warnings, 1)
	require.True(t, strings.HasPrefix(warnings[0], "body.md:3-6:"), "warning provenance: %q", warnings)
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

func TestAssemblePrivateCalloutBoundaries(t *testing.T) {
	const path = "docs/example.md"
	body := "Before.\n\n> [!mf-private] First\n> Sample one\n\n> [!mind-forge-private]\n> Sample two\n\nAfter.\n"
	got, warnings, err := AssembleBlocks([]Block{{Path: path, Content: body}}, false, "")
	require.NoError(t, err)
	require.Equal(t, "Before.\n\nAfter.\n", got)
	require.Empty(t, warnings)
}

func TestAssemblePrivateWarningSourceCoordinates(t *testing.T) {
	const path = "docs/example.md"
	body := "\nBefore.\n\n> [!mf-private] Sample title\n> Sample one\n\n> > [!mf-private] Nested title\n\n> Sample two\n\n> [!mf-private]\n> Sample three\n\n> Sample four\n\nAfter.\n"
	for _, tc := range []struct {
		name, front, kept, firstRange, secondRange string
	}{
		{"Typora", "---\ntypora-copy-images-to: assets\n---\n", "", "7-12", "14-17"},
		{"Mind Forge", "---\nmind-forge-visibility: public\n---\n", "", "7-12", "14-17"},
		{"Both with retained YAML", "---\ntitle: Example\ntypora-copy-images-to: assets\nmind-forge-visibility: public\n---\n", "---\ntitle: Example\n---\n\n", "9-14", "16-19"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			got, warnings, err := AssembleBlocks([]Block{{Path: path, Content: tc.front + body}}, false, "")
			require.NoError(t, err)
			require.Equal(t, tc.kept+"Before.\n\nAfter.\n", got)
			const explanation = ": quoted continuation after a bare blank line was removed as private"
			want := []string{
				path + ":" + tc.firstRange + ": mf-private callout \"Sample title\"" + explanation,
				path + ":" + tc.secondRange + ": mf-private callout" + explanation,
			}
			require.Equal(t, want, warnings)
		})
	}
}

func TestAssemblePrivateCalloutQuotedFence(t *testing.T) {
	const path = "docs/example.md"
	const start = "---\ntypora-copy-images-to: assets\n---\n\nBefore.\n\n> [!mf-private] Outer title\n> ~~~\n"
	const marker = "> [!mf-private] Synthetic fenced example\n"
	for _, tc := range []struct {
		name, quoted, location string
	}{
		{"closed", marker + "> ~~~\n\n> Continuation\n\n> [!mf-private] Next callout\n> Its body\n", "7-12"},
		{"unclosed", "\n" + marker + "> Continuation\n", "7-11"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			got, warnings, err := AssembleBlocks([]Block{{Path: path, Content: start + tc.quoted + "\nAfter.\n"}}, false, "")
			require.NoError(t, err)
			require.Equal(t, "Before.\n\nAfter.\n", got)
			require.Equal(t, []string{path + ":" + tc.location + ": mf-private callout \"Outer title\": quoted continuation after a bare blank line was removed as private"}, warnings)
			t.Logf("body=%q warnings=%q", got, warnings)
		})
	}
}

func TestAssemblePrivateCalloutsAfterYAMLScalarWithFenceMarker(t *testing.T) {
	source := "---\ndescription: |\n  ```\n---\n# Public title\n\n> [!mf-private] Notes\n> W37\n\n> W38\n\nFinal paragraph.\n"
	got, warnings, err := AssembleBlocks([]Block{{Path: "docs/post.md", Content: source}}, false, "")
	require.NoError(t, err)
	require.Contains(t, got, "description: |\n  ```")
	require.Contains(t, got, "Final paragraph.")
	require.NotContains(t, got, "W37")
	require.NotContains(t, got, "W38")
	require.Len(t, warnings, 1)
	require.Contains(t, warnings[0], "docs/post.md:7-10")
}
