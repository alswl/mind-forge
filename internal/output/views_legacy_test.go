package output

import (
	"strings"
	"testing"
)

func TestLegacyShowViews(t *testing.T) {
	t.Run("key_value_alignment", func(t *testing.T) {
		got := RenderShow(ShowBlock{Kind: "project", Identity: "demo", Fields: []ShowField{{Label: "Name", Value: ShowValue{Text: "demo"}}, {Label: "Path", Value: ShowValue{Text: "./projects/demo"}}}}, DisplayOptions{})
		lines := strings.Split(strings.TrimSuffix(got, "\n"), "\n")
		if len(lines) != 2 || strings.IndexByte(lines[0], ':') != strings.IndexByte(lines[1], ':') {
			t.Fatalf("key alignment changed: %q", got)
		}
	})
	t.Run("optional_shows_dash", func(t *testing.T) {
		got := RenderShow(ShowBlock{Kind: "source", Identity: "test", Fields: []ShowField{{Label: "Description", Value: ShowValue{Optional: true}}}}, DisplayOptions{})
		if !strings.Contains(got, "-") {
			t.Fatalf("optional placeholder absent: %q", got)
		}
	})
	t.Run("section_rendering", func(t *testing.T) {
		got := RenderShow(ShowBlock{Kind: "project", Identity: "demo", Sections: []ShowSection{{Heading: "Layout", Fields: []ShowField{{Label: "Articles", Value: ShowValue{Text: "docs"}}}}}}, DisplayOptions{})
		if !strings.Contains(got, "Layout:") || !strings.Contains(got, "Articles") {
			t.Fatalf("section absent: %q", got)
		}
	})
	t.Run("section_fields_participate_in_alignment", func(t *testing.T) {
		got := RenderShow(ShowBlock{Kind: "project", Identity: "demo", Fields: []ShowField{{Label: "Name", Value: ShowValue{Text: "demo"}}}, Sections: []ShowSection{{Heading: "Layout", Fields: []ShowField{{Label: "Build output", Value: ShowValue{Text: "public"}}}}}}, DisplayOptions{})
		lines := strings.Split(strings.TrimSuffix(got, "\n"), "\n")
		if len(lines) != 4 || strings.Index(lines[0], "demo") != strings.Index(lines[3], "public")-2 {
			t.Fatalf("section alignment changed: %q", got)
		}
	})
	t.Run("json_envelope_includes_kind_and_identity", func(t *testing.T) {
		got := ShowEnvelope(ShowBlock{Kind: "term", Identity: "RAG"}, map[string]any{"definition": "Retrieval..."})
		if got["kind"] != "term" || got["identity"] != "RAG" || got["definition"] != "Retrieval..." {
			t.Fatalf("show envelope changed: %#v", got)
		}
	})
	for _, test := range []struct {
		name    string
		options DisplayOptions
		linked  bool
	}{
		{"path_value_renders_as_link", DisplayOptions{RepoRoot: "/repo", Hyperlinks: true}, true},
		{"path_value_no_hyperlinks_returns_plain", DisplayOptions{}, false},
		{"path_value_no_repo_root_returns_plain", DisplayOptions{Hyperlinks: true}, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := RenderShow(ShowBlock{Kind: "article", Identity: "docs/readme", Fields: []ShowField{{Label: "Path", Value: ShowValue{Text: "docs/readme.md", Path: true}}}}, test.options)
			if !strings.Contains(got, "docs/readme.md") {
				t.Fatalf("visible path absent: %q", got)
			}
			if test.linked {
				if !strings.Contains(got, "\x1b]8;;file:///repo/docs/readme.md\x1b\\") {
					t.Fatalf("path link absent: %q", got)
				}
			} else if strings.ContainsRune(got, '\x1b') {
				t.Fatalf("plain path contains escapes: %q", got)
			}
		})
	}
}

func TestLegacyListViews(t *testing.T) {
	opts := ListOptions{Width: 80}
	t.Run("empty_list_shows_placeholder", func(t *testing.T) {
		if got := RenderList(List{Headers: []string{"NAME", "DOCS"}, Plural: "projects"}, opts); got != "No projects found.\n" {
			t.Fatalf("empty list = %q", got)
		}
	})
	t.Run("single_row_with_headers", func(t *testing.T) {
		got := RenderList(List{Headers: []string{"NAME", "DOCS"}, Rows: [][]ListCell{TextCells("demo", "4")}, Plural: "projects"}, opts)
		lines := strings.Split(strings.TrimSuffix(got, "\n"), "\n")
		if len(lines) != 2 || !strings.HasPrefix(lines[0], "NAME") || !strings.HasPrefix(lines[1], "demo") {
			t.Fatalf("list lines changed: %q", got)
		}
	})
	t.Run("optional_cell_shows_dash", func(t *testing.T) {
		got := RenderList(List{Headers: []string{"NAME", "LAST ACTIVITY"}, Rows: [][]ListCell{{{Text: "demo2"}, {Optional: true}}}, Plural: "projects"}, opts)
		if !strings.Contains(got, " -") {
			t.Fatalf("optional placeholder absent: %q", got)
		}
	})
	t.Run("truncation_applies_to_non_identity_columns", func(t *testing.T) {
		narrow := opts
		narrow.Width = 30
		got := RenderList(List{Headers: []string{"NAME", "DEFINITION"}, Rows: [][]ListCell{TextCells("RAG", "Retrieval-Augmented Generation for LLMs")}, Plural: "terms"}, narrow)
		if !strings.Contains(got, "RAG") || !(strings.Contains(got, "…") || strings.Contains(got, "...")) {
			t.Fatalf("identity/truncation changed: %q", got)
		}
	})
	t.Run("cjk_column_width", func(t *testing.T) {
		got := RenderList(List{Headers: []string{"TERM", "DEFINITION"}, Rows: [][]ListCell{TextCells("检索", "RAG的中文缩写")}, Plural: "terms"}, opts)
		if strings.TrimSpace(got) == "" {
			t.Fatal("CJK list empty")
		}
	})
	for _, test := range []struct {
		name    string
		display DisplayOptions
		linked  bool
	}{
		{"path_cell_renders_as_link", DisplayOptions{RepoRoot: "/home/user", Hyperlinks: true}, true},
		{"path_cell_no_hyperlinks_returns_plain", DisplayOptions{}, false},
		{"path_cell_no_repo_root_returns_plain", DisplayOptions{Hyperlinks: true}, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			local := opts
			local.DisplayOptions = test.display
			got := RenderList(List{Headers: []string{"PATH"}, Rows: [][]ListCell{{{Text: "docs/readme.md", Path: true}}}, Plural: "articles"}, local)
			if !strings.Contains(got, "docs/readme.md") {
				t.Fatalf("visible path absent: %q", got)
			}
			if test.linked {
				if !strings.Contains(got, "\x1b]8;;file:///home/user/docs/readme.md\x1b\\") {
					t.Fatalf("path link absent: %q", got)
				}
			} else if strings.ContainsRune(got, '\x1b') {
				t.Fatalf("plain list contains escapes: %q", got)
			}
		})
	}
}
