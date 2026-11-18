package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
)

func identityRepo(t *testing.T) string {
	t.Helper()
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: '.'\nprojects: []\n")
	return root
}

func TestFrozenIdentityProjectNormalization(t *testing.T) {
	for _, test := range []struct {
		name, cwd, input, want string
		refused                bool
	}{
		{"normalize_cwd_relative_with_slash", "workspaces/机器/projects", "2026-W21_装置", "workspaces/机器/projects/2026-W21_装置", false},
		{"normalize_repo_relative_with_slash_from_root", "", "workspaces/📊/projects/2026-W21_装置🚀", "workspaces/📊/projects/2026-W21_装置🚀", false},
		{"normalize_simple_name_legacy", "", "simple-report", "simple-report", false},
		{"normalize_rejects_empty_input", "", "", "", true},
		{"normalize_dot_from_repo_root_resolves_to_repo_root", "", ".", "", false},
		{"normalize_dot_from_inside_a_project_resolves_to_that_project", "projects/projA", ".", "projects/projA", false},
		{"normalize_rejects_dotdot_escape", "", "../escape", "", true},
		{"normalize_unicode_path_from_cwd", "workspaces/机器/projects", "2026-W21_装置", "workspaces/机器/projects/2026-W21_装置", false},
		{"normalize_emoji_path_from_root", "", "workspaces/📊/projects/2026-W21_装置🚀", "workspaces/📊/projects/2026-W21_装置🚀", false},
		{"reject_project_selector_escape_via_dotdot", "", "../outside", "", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := identityRepo(t)
			cwd := filepath.Join(root, test.cwd)
			if err := os.MkdirAll(cwd, 0755); err != nil {
				t.Fatal(err)
			}
			if test.want != "" {
				if err := os.MkdirAll(filepath.Join(root, test.want), 0755); err != nil {
					t.Fatal(err)
				}
			}
			result, err := NormalizeProjectTarget(root, cwd, test.input)
			if test.refused {
				if err == nil {
					t.Fatal("invalid selector accepted")
				}
				return
			}
			if err != nil || result.Path != test.want || result.RequestedPath != test.input || result.ResolvedPath != filepath.Join(root, test.want) {
				t.Fatalf("%#v %v", result, err)
			}
		})
	}
}

func TestFrozenIdentityEntityNormalization(t *testing.T) {
	for _, test := range []struct {
		name        string
		kind        EntityKind
		input, want string
		refused     bool
	}{
		{"normalize_article_selector_with_slash", Article, "docs/weekly.md", "docs/weekly.md", false},
		{"normalize_article_selector_simple_name", Article, "weekly", "docs/weekly", false},
		{"normalize_asset_selector_with_slash", Asset, "assets/chart.png", "assets/chart.png", false},
		{"normalize_source_selector_with_slash", Source, "sources/meeting/notes.md", "sources/meeting/notes.md", false},
		{"normalize_entity_rejects_project_root_escape", Article, "../outside.md", "", true},
		{"reject_entity_selector_escape_via_dotdot", Article, "../../etc/passwd", "", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			project := identityRepo(t)
			result, err := NormalizeEntitySelector(project, "my-project", test.kind, test.input)
			if test.refused {
				if err == nil {
					t.Fatal("escape accepted")
				}
				return
			}
			if err != nil || result.Kind != test.kind || result.ProjectIdentity != "my-project" || result.Path != test.want || result.ResolvedPath != filepath.Join(project, test.want) {
				t.Fatalf("%#v %v", result, err)
			}
		})
	}
}

func TestFrozenIdentityProjectDetection(t *testing.T) {
	for _, test := range []struct{ name, project, cwd string }{
		{"detect_project_from_nested_unicode_path", "workspaces/机器/projects/2026-W21_装置", ""},
		{"detect_project_from_nested_emoji_path", "workspaces/📊/projects/2026-W21_装置🚀", ""},
		{"detect_project_from_subdir", "my-project", "docs"},
		{"detect_project_returns_none_outside_project", "", ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := identityRepo(t)
			if test.project != "" {
				putContent(t, root, test.project+"/mind.yaml", "schema: '1'\n")
			}
			cwd := filepath.Join(root, test.project, test.cwd)
			if err := os.MkdirAll(cwd, 0755); err != nil {
				t.Fatal(err)
			}
			project, err := app.ResolveProject(root, nil, cwd, false)
			if test.project == "" {
				if err == nil {
					t.Fatal("invented project")
				}
				return
			}
			if err != nil || project != filepath.Join(root, test.project) {
				t.Fatalf("%s %v", project, err)
			}
		})
	}
}

func TestFrozenIdentityPathResolution(t *testing.T) {
	names := []string{"resolve_path_selector_cwd_relative_no_boundary", "resolve_path_selector_absolute_input_used_as_is", "resolve_path_selector_rejects_dotdot_before_boundary_check", "resolve_path_selector_rejects_empty_input", "resolve_path_selector_within_boundary_succeeds", "resolve_path_selector_outside_boundary_is_refused", "resolve_path_selector_project_flag_only_adds_a_check"}
	for _, name := range names {
		t.Run(name, func(t *testing.T) {
			root := identityRepo(t)
			project := filepath.Join(root, "projects/p")
			cwd := filepath.Join(root, "some/where")
			if err := os.MkdirAll(project, 0755); err != nil {
				t.Fatal(err)
			}
			if err := os.MkdirAll(cwd, 0755); err != nil {
				t.Fatal(err)
			}
			input := "target.md"
			var boundary *string
			switch name {
			case "resolve_path_selector_absolute_input_used_as_is":
				input = filepath.Join(root, "sources/file/x.md")
			case "resolve_path_selector_rejects_dotdot_before_boundary_check":
				input = "../escape.md"
				boundary = &root
			case "resolve_path_selector_rejects_empty_input":
				input = ""
			case "resolve_path_selector_within_boundary_succeeds", "resolve_path_selector_project_flag_only_adds_a_check":
				cwd = project
				input = "sources/yuque/renamed.md"
				boundary = &project
			case "resolve_path_selector_outside_boundary_is_refused":
				cwd = root
				input = "sources/yuque/renamed.md"
				boundary = &project
			}
			got, err := ResolvePathSelector(input, cwd, boundary, "target")
			if strings.Contains(name, "rejects") || name == "resolve_path_selector_outside_boundary_is_refused" {
				if err == nil {
					t.Fatal("unsafe target accepted")
				}
				return
			}
			want := input
			if !filepath.IsAbs(want) {
				want = filepath.Join(cwd, want)
			}
			if err != nil || got != want {
				t.Fatalf("%s want%s %v", got, want, err)
			}
			if name == "resolve_path_selector_project_flag_only_adds_a_check" {
				plain, err := ResolvePathSelector(input, cwd, nil, "target")
				if err != nil || plain != got {
					t.Fatal("boundary changed resolution")
				}
			}
		})
	}
}
