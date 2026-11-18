package build

import (
	"os"
	"path/filepath"
	"strings"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/services/content"
)

// PrepareRequest resolves the CLI selector and output destination before building.
func PrepareRequest(root, cwd, selector string, projectSelector, out *string, options Options) (*Prepared, error) {
	if selector == "" {
		return nil, output.Usage("invalid article name: ''", "use `mf article list` to see available articles")
	}
	var project string
	var err error
	if target, ok := strings.CutPrefix(selector, "@"); ok {
		if !filepath.IsAbs(target) {
			target = filepath.Join(root, target)
		}
		target, err = store.CanonicalWithin(root, target)
		if err != nil {
			return nil, err
		}
		options.Source = &target
		selector = strings.TrimSuffix(filepath.Base(target), filepath.Ext(target))
		project = target
		if info, err := os.Stat(target); err != nil {
			return nil, err
		} else if !info.IsDir() {
			project = filepath.Dir(target)
		}
		for {
			if _, err := os.Stat(filepath.Join(project, "mind.yaml")); err == nil {
				break
			}
			if project == root || filepath.Dir(project) == project {
				return nil, output.Usage("path '"+target+"' is not under a Mind Project", "choose a path below a directory containing mind.yaml")
			}
			project = filepath.Dir(project)
		}
	} else {
		project, err = content.ProjectFor(root, cwd, projectSelector, &selector)
		if err != nil {
			return nil, err
		}
	}
	if out := out; out != nil {
		path := *out
		if !filepath.IsAbs(path) {
			path = filepath.Join(cwd, path)
		}
		path, err = store.CanonicalWithin(string(filepath.Separator), path)
		if err != nil {
			return nil, err
		}
		options.Out = &path
	}
	p, err := Prepare(root, project, selector, options)
	if p != nil {
		p.Selector = selector
	}
	return p, err
}
