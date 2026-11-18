package config

import (
	"os"
	"path/filepath"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type ResolveOptions struct {
	CWD    string
	Root   string
	Config string
}

func canonical(path string) string {
	absolute, err := filepath.Abs(path)
	if err == nil {
		path = absolute
	}
	if resolved, err := filepath.EvalSymlinks(path); err == nil {
		return resolved
	}
	return path
}

func exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func DetectRoot(start string, maxDepth int) string {
	current := canonical(start)
	for depth := 0; ; depth++ {
		if exists(filepath.Join(current, "minds.yaml")) {
			return current
		}
		parent := filepath.Dir(current)
		if depth >= maxDepth || parent == current {
			return ""
		}
		current = parent
	}
}

func ResolveRoot(options ResolveOptions) (string, error) {
	relativeToCWD := func(path string) string {
		if filepath.IsAbs(path) {
			return path
		}
		return filepath.Join(options.CWD, path)
	}
	if options.Root != "" {
		root := canonical(relativeToCWD(options.Root))
		if !exists(filepath.Join(root, "minds.yaml")) {
			return "", output.NotInRepo()
		}
		return root, nil
	}
	if options.Config != "" {
		config := relativeToCWD(options.Config)
		info, err := os.Stat(config)
		if err != nil {
			return "", nil
		}
		if !info.IsDir() {
			config = filepath.Dir(config)
		}
		if exists(filepath.Join(config, "minds.yaml")) {
			return canonical(config), nil
		}
		return "", nil
	}
	return DetectRoot(options.CWD, 50), nil
}
