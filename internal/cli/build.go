package cli

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/build"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"github.com/spf13/cobra"
)

func buildCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	selector := args[0]
	if selector == "" {
		return output.Usage("invalid article name: ''", "use `mf article list` to see available articles")
	}
	options := build.Options{Format: stringFlag(c, "format"), DryRun: boolFlag(c, "dry-run")}
	var project string
	var err error
	if target, ok := strings.CutPrefix(selector, "@"); ok {
		if !filepath.IsAbs(target) {
			target = filepath.Join(a.RepoRoot, target)
		}
		target, err = store.CanonicalWithin(a.RepoRoot, target)
		if err != nil {
			return err
		}
		options.Source = &target
		selector = strings.TrimSuffix(filepath.Base(target), filepath.Ext(target))
		project = target
		if info, err := os.Stat(target); err != nil {
			return err
		} else if !info.IsDir() {
			project = filepath.Dir(target)
		}
		for {
			if _, err := os.Stat(filepath.Join(project, "mind.yaml")); err == nil {
				break
			}
			if project == a.RepoRoot || filepath.Dir(project) == project {
				return output.Usage("path '"+target+"' is not under a Mind Project", "choose a path below a directory containing mind.yaml")
			}
			project = filepath.Dir(project)
		}
	} else {
		project, err = content.ProjectFor(a.RepoRoot, a.CWD, optionalFlag(c, "project"), &selector)
		if err != nil {
			return err
		}
	}
	if out := optionalFlag(c, "out"); out != nil {
		path := *out
		if !filepath.IsAbs(path) {
			path = filepath.Join(a.CWD, path)
		}
		path, err = store.CanonicalWithin(string(filepath.Separator), path)
		if err != nil {
			return err
		}
		options.Out = &path
	}
	p, err := build.Prepare(a.RepoRoot, project, selector, options)
	if err != nil {
		return err
	}
	if options.DryRun {
		data, err := jsonObject(p.Plan)
		if err != nil {
			return err
		}
		data["location"] = p.Location
		lines := []string{"Build Plan: " + p.Plan.Article, "  Input sources:"}
		for i, file := range p.Plan.InputFiles {
			lines = append(lines, fmt.Sprintf("    %d. %s (%.1f KB)", i+1, file.Path, float64(file.Size)/1024))
		}
		order := []string{}
		for _, part := range p.Plan.MergeOrder {
			order = append(order, strconv.Quote(part))
		}
		lines = append(lines, "  Merge order: ["+strings.Join(order, ", ")+"]", "  Output path: "+p.Plan.OutputPath, fmt.Sprintf("  Estimated size: %.1f KB", float64(p.Plan.EstimatedSize)/1024))
		if p.Plan.Banner != nil {
			level := "raw"
			if p.Plan.Banner.Level != nil {
				level = *p.Plan.Banner.Level
			}
			lines = append(lines, "  Banner: enabled, level="+level)
		}
		for _, stage := range p.Plan.Pipeline {
			lines = append(lines, fmt.Sprintf("  Pipeline: %s %s -> %s [%s]", stage.Rule, stage.Input, stage.Output, stage.Action))
		}
		return renderTerm(a, stdout, stderr, data, strings.Join(lines, "\n"), nil)
	}
	size, warnings, err := p.Execute(a.RepoRoot, nil)
	if err != nil {
		return err
	}
	data := map[string]any{"article": selector, "location": p.Location, "format": p.Plan.Format, "output": p.Plan.OutputPath, "size_bytes": size}
	return renderTerm(a, stdout, stderr, data, fmt.Sprintf("Article built: %s\n  Output: %s\n  Size: %.1f KB", selector, p.Plan.OutputPath, float64(size)/1024), warnings)
}
