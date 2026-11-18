package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
)

func renderAssetUpdates(stdout io.Writer, application *app.Context, items model.JSONArray[model.AssetUpdateResult], dryRun bool) error {
	if application.Quiet {
		return nil
	}
	changed, missing := 0, 0
	lines := []string{}
	if dryRun {
		lines = append(lines, "[dry-run] would update assets:")
	}
	for _, item := range items {
		if item.Changed {
			changed++
		}
		if item.Error != nil {
			missing++
		}
		switch {
		case item.Error != nil:
			lines = append(lines, "✗ missing  "+item.Path)
		case item.Changed:
			lines = append(lines, "✓ updated "+item.Path)
		default:
			lines = append(lines, "= unchanged "+item.Path)
		}
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", map[string]any{"kind": "asset", "dry_run": dryRun, "items": items, "summary": map[string]any{"total": len(items), "changed": changed, "missing": missing}})
	}
	lines = append(lines, fmt.Sprintf("%d assets, %d changed, %d missing", len(items), changed, missing))
	return output.RenderRaw(stdout, "text", strings.Join(lines, "\n"))
}
