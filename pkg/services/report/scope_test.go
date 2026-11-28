package report

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestProjectScopeAndScaffold(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		t.Run(layout, func(t *testing.T) {
			root, err := canonicalDir(t.TempDir())
			require.NoError(t, err)
			writeFile(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [lab]\n")
			writeFile(t, root, "projects/lab/mind.yaml", "name: lab\ncontent_layout: "+layout+"\nlayout:\n  templates: scaffolds\n")
			project := "lab"
			scope, err := ProjectScope(root, root, &project)
			require.NoError(t, err)
			service := Service{Registry: root}
			result, err := service.Scaffold(t.Context(), scope, "weekly", "builtin/trend")
			require.NoError(t, err)
			require.Equal(t, "scaffolds/reports/weekly", result.Directory)
			plan, err := service.Plan(t.Context(), scope, Request{Slug: "weekly"})
			require.NoError(t, err)
			require.Equal(t, "builtin/trend", plan.Renderer)
			_, err = service.Scaffold(t.Context(), scope, "weekly", "builtin/brief")
			require.ErrorIs(t, err, ErrConflict)
			_, err = service.Scaffold(t.Context(), scope, "Weekly", "")
			require.ErrorIs(t, err, ErrInvalid)
			_, err = service.Plan(t.Context(), scope, Request{Slug: "../weekly"})
			require.ErrorIs(t, err, ErrInvalid)
			_, err = service.Plan(t.Context(), scope, Request{Slug: "missing"})
			require.ErrorIs(t, err, ErrNotFound)

			writeFile(t, root, "projects/lab/mind.yaml", "name: lab\nlayout:\n  templates: scaffolds\n  articles: scaffolds/reports/articles\n")
			_, err = ProjectScope(root, root, &project)
			require.ErrorIs(t, err, ErrInvalid)
			require.ErrorContains(t, err, "overlaps")
		})
	}
}

func TestPlanRejectsCaseAliasesSymlinksAndTraversal(t *testing.T) {
	scope, request := standalone(t)
	request.Entry = "Report.tsx"
	_, err := (Service{}).Plan(t.Context(), scope, request)
	require.ErrorContains(t, err, "case mismatch")
	require.NoError(t, os.Symlink("report.tsx", filepath.Join(scope.Directory, "link.tsx")))
	request.Entry = "link.tsx"
	_, err = (Service{}).Plan(t.Context(), scope, request)
	require.ErrorContains(t, err, "symlinks")
	request.Entry = "outputs/../report.tsx"
	_, err = (Service{}).Plan(t.Context(), scope, request)
	require.ErrorContains(t, err, "parent traversal")
}
