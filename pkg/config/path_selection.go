package config

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

// SelectProjectFromPath adopts only an existing path inside a project. Bare
// names and explicit project selections retain the command's normal resolution.
func SelectProjectFromPath(root, cwd, input string, explicit bool) (string, string, bool) {
	if explicit || root == "" || !strings.ContainsAny(input, "/\\") && !strings.HasPrefix(input, ".") {
		return "", "", false
	}
	if err := store.RejectTraversal(input, "repo"); err != nil {
		return "", "", false
	}
	candidates := []string{input}
	if !filepath.IsAbs(input) {
		candidates = []string{filepath.Join(cwd, input), filepath.Join(root, input)}
	}
	path := ""
	for _, candidate := range candidates {
		if !exists(candidate) {
			continue
		}
		canonical, err := store.CanonicalWithin(root, candidate)
		if err != nil {
			return "", "", false
		}
		path = canonical
		break
	}
	if path == "" {
		return "", "", false
	}
	current := path
	if info, err := os.Stat(current); err != nil {
		return "", "", false
	} else if !info.IsDir() {
		current = filepath.Dir(current)
	}
	boundary := canonical(root)
	for {
		if exists(filepath.Join(current, "mind.yaml")) {
			relative, err := filepath.Rel(current, path)
			if err != nil || relative == "." {
				return "", "", false
			}
			return current, filepath.ToSlash(relative), true
		}
		if current == boundary || filepath.Dir(current) == current {
			return "", "", false
		}
		current = filepath.Dir(current)
	}
}
