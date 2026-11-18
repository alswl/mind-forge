package architecture

import (
	"go/parser"
	"go/token"
	"io/fs"
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
	const module = "github.com/alswl/mind-forge/"
	allowed := map[string][]string{
		"cmd":      {"pkg/services/", "pkg/config", "pkg/common/", "internal/output", "internal/version"},
		"services": {"pkg/services/", "pkg/managers/", "pkg/dal/", "pkg/common/", "pkg/config"},
		"managers": {"pkg/managers/", "pkg/dal/", "pkg/common/"},
		"dal":      {"pkg/dal/", "pkg/common/"},
		"config":   {"pkg/config", "pkg/dal/", "pkg/common/"},
		"common":   {"pkg/common/"},
	}
	for _, tree := range []string{"cmd", "pkg"} {
		err := filepath.WalkDir(filepath.Join(root, tree), func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() || !strings.HasSuffix(path, ".go") || strings.HasSuffix(path, "_test.go") {
				return nil
			}
			relative, err := filepath.Rel(root, path)
			require.NoError(t, err)
			parts := strings.Split(filepath.ToSlash(relative), "/")
			layer := parts[0]
			if layer == "pkg" {
				layer = parts[1]
			}
			permits, exists := allowed[layer]
			require.True(t, exists, "unknown layer: %s", relative)
			source, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
			require.NoError(t, err)
			for _, dependency := range source.Imports {
				imported, err := strconv.Unquote(dependency.Path.Value)
				require.NoError(t, err)
				if !strings.HasPrefix(imported, module) {
					continue
				}
				local := strings.TrimPrefix(imported, module)
				valid := false
				for _, prefix := range permits {
					valid = valid || strings.HasPrefix(local, prefix)
				}
				require.True(t, valid, "%s imports forbidden layer %s", relative, local)
			}
			return nil
		})
		require.NoError(t, err)
	}
}
