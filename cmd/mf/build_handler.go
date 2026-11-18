package main

import (
	"fmt"
	"io"
	"strconv"
	"strings"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/build"
	"github.com/spf13/cobra"
)

func buildCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	options := build.Options{Context: c.Context(), Format: stringFlag(c, "format"), DryRun: boolFlag(c, "dry-run")}
	p, err := build.PrepareRequest(a.RepoRoot, a.CWD, args[0], optionalFlag(c, "project"), optionalFlag(c, "out"), options)
	if err != nil {
		return err
	}

	selector := p.Selector
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
