package architecture

import (
	"fmt"
	"go/build"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

// Keep the guide's layer boundaries enforceable as new packages are added.
func TestProductionDependenciesPointDownward(t *testing.T) {
	_, file, _, ok := runtime.Caller(0)
	require.True(t, ok)
	root := filepath.Clean(filepath.Join(filepath.Dir(file), "../../.."))
	violations, err := dependencyViolations(root)
	require.NoError(t, err)
	require.Empty(t, violations)
}

func dependencyViolations(root string) ([]string, error) {
	violations := []string{}
	const module = "github.com/alswl/mind-forge/"
	allowed := map[string][]string{
		"cmd":                {"pkg/services/", "pkg/config", "pkg/common/", "internal/output", "internal/version", "internal/reportcli", "pkg/server/report"},
		"server":             {"pkg/services/report"},
		"internal/reportcli": {"pkg/server/report", "pkg/services/report", "internal/output"},
		"services":           {"pkg/services/", "pkg/managers/", "pkg/dal/", "pkg/common/", "pkg/config"},
		"managers":           {"pkg/managers/", "pkg/dal/", "pkg/common/"},
		"dal":                {"pkg/dal/", "pkg/common/"},
		"config":             {"pkg/config", "pkg/dal/", "pkg/common/"},
		"common":             {"pkg/common/"},
		"internal/output":    {"pkg/common/"},
		"internal/version":   {},
	}
	for _, tree := range []string{"cmd", "pkg", "internal"} {
		err := filepath.WalkDir(filepath.Join(root, tree), func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() || !strings.HasSuffix(path, ".go") || strings.HasSuffix(path, "_test.go") {
				return nil
			}
			relative, err := filepath.Rel(root, path)
			if err != nil {
				return err
			}
			parts := strings.Split(filepath.ToSlash(relative), "/")
			layer := parts[0]
			if layer == "pkg" {
				layer = parts[1]
			} else if layer == "internal" {
				layer = strings.Join(parts[:2], "/")
			}
			permits, exists := allowed[layer]
			if !exists {
				return fmt.Errorf("unknown layer: %s", relative)
			}
			source, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
			if err != nil {
				return err
			}
			for _, dependency := range source.Imports {
				imported, err := strconv.Unquote(dependency.Path.Value)
				if err != nil {
					return err
				}
				if layer == "internal/version" {
					pkg, err := build.Default.Import(imported, "", build.FindOnly)
					if err != nil || !pkg.Goroot {
						violations = append(violations, fmt.Sprintf("%s imports forbidden layer %s", relative, strings.TrimPrefix(imported, module)))
					}
					continue
				}
				if !strings.HasPrefix(imported, module) {
					continue
				}
				local := strings.TrimPrefix(imported, module)
				valid := false
				for _, prefix := range permits {
					allowedPackage := strings.TrimSuffix(prefix, "/")
					valid = valid || local == allowedPackage || strings.HasPrefix(local, allowedPackage+"/")
				}
				if !valid {
					violations = append(violations, fmt.Sprintf("%s imports forbidden layer %s", relative, local))
				}
			}
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	return violations, nil
}

func TestDependencyGuardDetectsForbiddenImports(t *testing.T) {
	tests := []struct{ name, path, dependency string }{
		{"internal reverse", "internal/output/forbidden.go", "pkg/services/content"},
		{"config pseudo prefix", "cmd/mf/forbidden.go", "pkg/configuration"},
		{"output pseudo prefix", "cmd/mf/forbidden.go", "internal/outputfoo"},
		{"version project import", "internal/version/forbidden.go", "pkg/common/model"},
		{"version external import", "internal/version/forbidden.go", "example.com/nonstdlib"},
		{"tagged source", "internal/output/forbidden.go", "pkg/config"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := dependencyFixture(t)
			path := filepath.Join(root, tc.path)
			require.NoError(t, os.MkdirAll(filepath.Dir(path), 0755))
			dependency := "github.com/alswl/mind-forge/" + tc.dependency
			if tc.name == "version external import" {
				dependency = tc.dependency
			}
			source := fmt.Sprintf("package fixture\nimport _ %q\n", dependency)
			if tc.name == "tagged source" {
				source = "//go:build synthetic_unavailable\n\n" + source
			}
			require.NoError(t, os.WriteFile(path, []byte(source), 0644))
			violations, err := dependencyViolations(root)
			require.NoError(t, err)
			require.Equal(t, []string{tc.path + " imports forbidden layer " + tc.dependency}, violations)
		})
	}
}

func TestDependencyGuardAllowsExactPackagesAndSubtrees(t *testing.T) {
	root := dependencyFixture(t)
	for path, dependency := range map[string]string{
		"cmd/mf/config.go": "pkg/config", "cmd/mf/outputchild.go": "internal/output/terminal", "cmd/mf/output.go": "internal/output", "pkg/config/nested/value.go": "pkg/common/model",
		"internal/output/value.go": "pkg/common/model", "pkg/services/query/value.go": "pkg/dal/store",
	} {
		full := filepath.Join(root, path)
		require.NoError(t, os.MkdirAll(filepath.Dir(full), 0755))
		require.NoError(t, os.WriteFile(full, []byte("package fixture\nimport _ \"github.com/alswl/mind-forge/"+dependency+"\"\n"), 0644))
	}
	versionPath := filepath.Join(root, "internal/version/value.go")
	require.NoError(t, os.MkdirAll(filepath.Dir(versionPath), 0755))
	require.NoError(t, os.WriteFile(versionPath, []byte(`package version
import _ "fmt"
`), 0644))
	violations, err := dependencyViolations(root)
	require.NoError(t, err)
	require.Empty(t, violations)
}

func dependencyFixture(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	for _, tree := range []string{"cmd", "pkg", "internal"} {
		require.NoError(t, os.Mkdir(filepath.Join(root, tree), 0755))
	}
	return root
}
