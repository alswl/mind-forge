package content

import (
	"strings"
	"testing"
)

func TestLegacyFilenameUtility(t *testing.T) {
	for _, test := range []struct {
		name  string
		cases [][2]string
	}{
		{"test_to_filename_ascii_unchanged", [][2]string{{"Hello World", "hello-world"}, {"Foo_Bar.Baz", "foo-bar-baz"}, {"  --a--  ", "a"}}},
		{"test_to_filename_empty_is_untitled", [][2]string{{"!!!", "untitled"}, {"", "untitled"}}},
		{"test_to_filename_preserves_cjk", [][2]string{{"本周进展", "本周进展"}, {"里程碑规划与进展", "里程碑规划与进展"}, {"Agent 建站周报", "agent-建站周报"}}},
	} {
		t.Run(test.name, func(t *testing.T) {
			for _, c := range test.cases {
				if got := ArticleFilename(c[0]); got != c[1] {
					t.Fatalf("filename(%q) = %q, want %q", c[0], got, c[1])
				}
			}
			if test.name == "test_to_filename_preserves_cjk" && ArticleFilename("本周进展") == ArticleFilename("里程碑规划与进展") {
				t.Fatal("distinct CJK headings collapsed")
			}
		})
	}
}

func TestLegacyProjectNameValidation(t *testing.T) {
	for _, test := range []struct {
		name, input string
		bad         bool
	}{
		{"test_validate_name_empty", "", true},
		{"test_validate_name_dot", ".", true},
		{"test_validate_name_dotdot", "..", true},
		{"test_validate_name_contains_slash", "foo/bar", true},
		{"test_validate_name_contains_backslash", `foo\bar`, true},
		{"test_validate_name_uppercase", "MyProject", true},
		{"test_validate_name_valid_kebab", "my-project", false},
		{"test_validate_name_valid_alphanumeric", "project42", false},
		{"test_validate_name_with_underscore", "my_project", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := validateProjectName(test.input)
			if (err != nil) != test.bad {
				t.Fatalf("name validation = %v; reject=%v", err, test.bad)
			}
			if test.input == "" && !strings.Contains(err.Error(), "empty") {
				t.Fatalf("missing empty-name diagnostic: %v", err)
			}
		})
	}
}
