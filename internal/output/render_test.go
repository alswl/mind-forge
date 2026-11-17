package output

import (
	"bytes"
	"errors"
	"io"
	"strings"
	"testing"
)

func TestRawAndErrorRendering(t *testing.T) {
	for _, item := range []struct{ input, want string }{
		{`[1,2]`, "{\"command\":\"mf\",\"data\":[1,2],\"status\":\"ok\"}\n"},
		{"plain", "{\"command\":\"mf\",\"data\":\"plain\",\"status\":\"ok\"}\n"},
	} {
		var buffer bytes.Buffer
		if err := RenderRaw(&buffer, "json", item.input); err != nil || buffer.String() != item.want {
			t.Fatalf("raw rendering: %q (%v)", buffer.String(), err)
		}
	}
	var buffer bytes.Buffer
	if err := RenderError(&buffer, "text", Usage("invalid", "try again")); err != nil {
		t.Fatal(err)
	}
	if buffer.String() != "error: invalid\n\nHint: try again\n\nRun `mf --help` for usage.\n" {
		t.Fatal(buffer.String())
	}
}

func TestShowAlignmentAndEncodedHyperlinks(t *testing.T) {
	block := ShowBlock{Kind: "source", Identity: "example", Fields: []ShowField{{Label: "名", Value: ShowValue{Text: "example"}}, {Label: "Optional", Value: ShowValue{Optional: true}}}, Sections: []ShowSection{{Heading: "Files", Fields: []ShowField{{Label: "Path", Value: ShowValue{Text: "my docs/设计.md", Path: true}}}}}}
	plain := RenderShow(block, DisplayOptions{})
	if plain != "名:        example\nOptional:  -\n\nFiles:\n  Path:      my docs/设计.md\n" {
		t.Fatalf("show alignment: %q", plain)
	}
	rich := RenderShow(block, DisplayOptions{RepoRoot: "/repo", Hyperlinks: true})
	if !strings.Contains(rich, "\x1b]8;;file:///repo/my%20docs/%E8%AE%BE%E8%AE%A1.md\x1b\\my docs/设计.md\x1b]8;;\x1b\\") {
		t.Fatalf("encoded path link: %q", rich)
	}
}

func TestConfirmationUsesInjectedTerminalAndDoesNotReadWhenPreconfirmed(t *testing.T) {
	for _, input := range []struct {
		answer    string
		confirmed bool
	}{{"YES\n", true}, {" y \n", true}, {"n\n", false}, {"", false}} {
		var prompt bytes.Buffer
		err := RequireConfirmation(ConfirmArgs{VerbLabel: "removal", Kind: "article", Identity: "docs/example"}, &prompt, func() (io.ReadCloser, error) { return io.NopCloser(strings.NewReader(input.answer)), nil })
		if (err == nil) != input.confirmed || prompt.String() != "Confirm removal of article \"docs/example\"? [y/N] " {
			t.Fatalf("confirmation %q: %q (%v)", input.answer, prompt.String(), err)
		}
	}
	open := func() (io.ReadCloser, error) { return nil, errors.New("no tty") }
	if err := RequireConfirmation(ConfirmArgs{Identity: "example"}, io.Discard, open); err == nil || !strings.Contains(err.Error(), "requires confirmation") {
		t.Fatalf("non-TTY allowed: %v", err)
	}
	if err := RequireConfirmation(ConfirmArgs{Yes: true}, io.Discard, func() (io.ReadCloser, error) { t.Fatal("preconfirmed operation opened terminal"); return nil, nil }); err != nil {
		t.Fatal(err)
	}
}

func TestTerminalCapabilityAndPolicyPrecedence(t *testing.T) {
	for _, item := range []struct {
		env        map[string]string
		tty        bool
		mode       string
		hyperlinks bool
		source     string
	}{
		{map[string]string{"TERM": "xterm-ghostty"}, false, "none", false, "disabled"},
		{map[string]string{"TERM": "xterm-ghostty", "MF_FORCE_TTY": "1", "NO_COLOR": "1"}, false, "none", false, "disabled"},
		{map[string]string{"TERM": "xterm-ghostty", "MF_NO_HYPERLINKS": "1", "MF_FORCE_HYPERLINKS": "1"}, true, "truecolor", false, "terminal_identity"},
		{map[string]string{"TERM": "tmux-256color"}, true, "truecolor", true, "terminal_identity"},
		{map[string]string{"TERM": "screen-256color"}, true, "ansi256", true, "terminal_identity"},
		{map[string]string{"TERM": "dumb"}, true, "none", false, "fallback"},
		{map[string]string{"TERM": "xterm-direct"}, true, "truecolor", false, "terminfo"},
		{map[string]string{}, true, "ansi16", false, "fallback"},
	} {
		profile := BuildTerminalProfile(CapabilityInput{Env: item.env, StdoutIsTTY: item.tty, Width: 80})
		if profile.ColorMode != item.mode || profile.Hyperlinks != item.hyperlinks || profile.DetectionSource != item.source {
			t.Fatalf("capability for %#v: %#v", item.env, profile)
		}
		policy := RenderingPolicy(profile, "json")
		if !policy.PlainOutput || policy.EmitANSIColor || policy.EmitHyperlinks || policy.EmitTruecolor {
			t.Fatalf("JSON rich output: %#v", policy)
		}
	}
}

func TestVerbTextAndEnvelopeRetainPerVerbShapes(t *testing.T) {
	old, path := "old", "sources/new.md"
	renamed := VerbResult{Verb: Rename, Kind: "source", Identity: "new", OldIdentity: &old, Path: &path, DryRun: true, Details: map[string]any{"changes": map[string]any{}}}
	if got := RenderVerb(renamed, DisplayOptions{}); got != "[dry-run] would rename source: old → new (sources/new.md)" {
		t.Fatal(got)
	}
	if envelope := VerbEnvelope(renamed); envelope["old_identity"] != "old" || envelope["new_identity"] != "new" || envelope["path"] != path {
		t.Fatal(envelope)
	}
	indexed := VerbResult{Verb: Index, Kind: "article", Details: map[string]any{"added": []string{"a"}, "removed": []string{"b"}, "kept_count": uint64(5), "kind": "ignored"}}
	if got := RenderVerb(indexed, DisplayOptions{}); got != "indexed article: +1 =5 -1" {
		t.Fatal(got)
	}
	envelope := VerbEnvelope(indexed)
	if envelope["kind"] != "article" || envelope["kept_count"] != uint64(5) {
		t.Fatal(envelope)
	}
	if _, present := envelope["details"]; present {
		t.Fatal("index details must be flattened")
	}
	lint := VerbResult{Verb: Lint, Kind: "article", Details: map[string]any{"issues": []any{map[string]any{"severity": "error", "kind": "missing_directory", "message": "missing", "path": "docs/a", "line": uint64(42)}}, "summary": map[string]any{"errors": uint64(1)}}}
	if got := RenderVerb(lint, DisplayOptions{RepoRoot: "/repo", Hyperlinks: true}); !strings.Contains(got, "\x1b]8;;\x1b\\:42") || !strings.HasSuffix(got, "1 errors, 0 warnings, 0 info\n") {
		t.Fatal(got)
	}
}

func TestListRenderingSeparatesVisibleWidthFromTerminalEscapes(t *testing.T) {
	view := List{Headers: []string{"NAME", "PATH", "STATE"}, Rows: [][]ListCell{{{Text: "a"}, {Text: "docs/example.md", Path: true}, {Text: "ready", ANSIPrefix: "\x1b[32m"}}}, Plural: "items"}
	plain := ListOptionsFromProfile(BuildTerminalProfile(CapabilityInput{Width: 80}), false, false)
	if !plain.NoHeaders || !plain.NoTrunc || plain.ColorEnabled || plain.Hyperlinks {
		t.Fatalf("non-TTY list policy: %#v", plain)
	}
	if got := RenderList(view, plain); got != "a     docs/example.md  ready\n" {
		t.Fatalf("plain list: %q", got)
	}
	rich := ListOptions{NoHeaders: true, NoTrunc: true, ColorEnabled: true, DisplayOptions: DisplayOptions{RepoRoot: "/repo", Hyperlinks: true}}
	got := RenderList(view, rich)
	if !strings.HasPrefix(got, "a     \x1b]8;;file:///repo/docs/example.md") || !strings.HasSuffix(got, "\x1b[32mready\x1b[0m\n") {
		t.Fatalf("rich list: %q", got)
	}
}

type failingWriter struct{}

func (failingWriter) Write([]byte) (int, error) { return 0, io.ErrClosedPipe }

func TestOutputWriterFailuresAndWarningOrder(t *testing.T) {
	for _, render := range []func() error{
		func() error { return RenderRaw(failingWriter{}, "text", "raw") },
		func() error { return RenderRaw(failingWriter{}, "json", `{}`) },
		func() error { return RenderError(failingWriter{}, "text", Usage("invalid", "")) },
		func() error { return RenderSuccess(failingWriter{}, "json", map[string]any{}) },
	} {
		if err := render(); !errors.Is(err, io.ErrClosedPipe) {
			t.Fatalf("writer failure swallowed: %v", err)
		}
	}
	var buffer bytes.Buffer
	warnings := []string{}
	EmitWarning(&buffer, &warnings, "first")
	EmitWarning(&buffer, &warnings, "second")
	if buffer.String() != "WARN: first\nWARN: second\n" || strings.Join(warnings, ",") != "first,second" {
		t.Fatalf("warning stream/order: %q %#v", buffer.String(), warnings)
	}
	buffer.Reset()
	if err := RenderSuccess(&buffer, "text", map[string]any{"名": "value"}); err != nil || buffer.String() != "  名  value\n" {
		t.Fatalf("legacy generic field padding: %q (%v)", buffer.String(), err)
	}
}
