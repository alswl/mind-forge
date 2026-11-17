package store

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
)

func RejectTraversal(input, scope string) error {
	for _, component := range strings.FieldsFunc(input, func(char rune) bool { return char == '/' || char == '\\' }) {
		if component == ".." {
			return output.Usage(fmt.Sprintf("path '%s' contains '..', which is not accepted in a %s selector", input, scope), "use a path under the "+scope+" root without '..' components")
		}
	}
	return nil
}

// CanonicalWithin resolves the existing prefix of a destination before
// checking its boundary, including destinations whose parents do not exist.
func CanonicalWithin(root, target string) (string, error) {
	canonicalRoot, err := filepath.EvalSymlinks(root)
	if err != nil {
		return "", err
	}
	canonicalRoot, err = filepath.Abs(canonicalRoot)
	if err != nil {
		return "", err
	}
	resolved, err := filepath.Abs(target)
	if err != nil {
		return "", err
	}
	var missing []string
	for {
		canonical, resolveError := filepath.EvalSymlinks(resolved)
		if resolveError == nil {
			resolved = canonical
			break
		}
		if !errors.Is(resolveError, os.ErrNotExist) {
			return "", resolveError
		}
		// A dangling symlink is an existing entry, not an absent destination.
		if _, statError := os.Lstat(resolved); statError == nil {
			return "", resolveError
		}
		parent := filepath.Dir(resolved)
		if parent == resolved {
			return "", resolveError
		}
		missing = append(missing, filepath.Base(resolved))
		resolved = parent
	}
	for i := len(missing) - 1; i >= 0; i-- {
		resolved = filepath.Join(resolved, missing[i])
	}
	relative, err := filepath.Rel(canonicalRoot, resolved)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", output.Usage(fmt.Sprintf("path '%s' is outside the boundary root '%s'", target, canonicalRoot), "use a path under the repo root")
	}
	return resolved, nil
}
