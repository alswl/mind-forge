package render

import (
	"strings"
	"testing"
)

func TestRenderFrozenAssertions(t *testing.T) {
	for _, test := range []struct {
		name, value string
		valid       bool
	}{
		{"test_validate_template_name_empty", "", false},
		{"test_validate_template_name_with_slash", "foo/bar", false},
		{"test_validate_template_name_valid", "report", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := validateTemplateName(test.value); (err == nil) != test.valid {
				t.Fatalf("%q: %v", test.value, err)
			}
		})
	}
	t.Run("test_built_in_templates_include_report_and_paper", func(t *testing.T) {
		items, err := Templates(t.TempDir())
		if err != nil {
			t.Fatal(err)
		}
		report, paper := false, false
		for _, item := range items {
			if item.Info.Name == "report" {
				report = item.Info.Default
			}
			paper = paper || item.Info.Name == "paper"
		}
		if !report || !paper {
			t.Fatalf("built-ins: %+v", items)
		}
	})
	t.Run("test_assemble_prompt_contains_output_boundaries", func(t *testing.T) {
		prompt := AssemblePrompt("Render this content.", []Content{{Path: "outputs/test.md", Body: "# Hello", Size: 8}}, "")
		for _, part := range []string{"--- BEGIN MF OUTPUT: outputs/test.md ---", "--- END MF OUTPUT: outputs/test.md ---", "HTML", "mf is NOT rendering"} {
			if !strings.Contains(prompt, part) {
				t.Fatalf("missing %q: %s", part, prompt)
			}
		}
	})
	for _, test := range []struct{ name, form, want string }{
		{"test_assemble_prompt_html_form_document", "document", "complete HTML document"},
		{"test_assemble_prompt_html_form_fragment", "fragment", "HTML fragment"},
	} {
		t.Run(test.name, func(t *testing.T) {
			prompt := AssemblePrompt("test", []Content{{Path: "outputs/t.md", Body: "content", Size: 7}}, test.form)
			if !strings.Contains(prompt, test.want) {
				t.Fatal(prompt)
			}
		})
	}
	t.Run("test_parse_frontmatter_with_label_and_description", func(t *testing.T) {
		label, description, body := ParseTemplate("---\nlabel: Team Review\ndescription: Internal review page\n---\n\nTemplate body", "team-review")
		if label != "Team Review" || description != "Internal review page" || strings.TrimSpace(body) != "Template body" {
			t.Fatalf("%q %q %q", label, description, body)
		}
	})
	t.Run("test_parse_frontmatter_without_frontmatter", func(t *testing.T) {
		label, description, body := ParseTemplate("Just content", "fallback")
		if label != "fallback" || description != "" || body != "Just content" {
			t.Fatalf("%q %q %q", label, description, body)
		}
	})
}
