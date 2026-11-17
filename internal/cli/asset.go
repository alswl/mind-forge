package cli

import (
	"fmt"
	"io"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/asset"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func assetCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, command.Name() == "new")
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "asset", DryRun: dryRun, Details: map[string]any{}}
	switch command.Name() {
	case "list", "show":
		return assetReadCommand(command, &view, args, stdout)
	case "new":
		result.Verb = output.Add
		if dryRun {
			name := filepath.Base(args[0])
			if name == "." || name == "/" {
				name = "unknown"
			}
			if value := optionalFlag(command, "name"); value != nil {
				name = *value
			}
			result.Identity, result.Path = name, &name
			result.Details["source"] = args[0]
			break
		}
		tags, _ := command.Flags().GetStringArray("tag")
		item, err := service.Add(project, application.CWD, args[0], asset.AddOptions{Name: optionalFlag(command, "name"), Tags: tags, Link: boolFlag(command, "link"), Force: boolFlag(command, "force")})
		if err != nil {
			return err
		}
		result.Identity, result.Path = item.Path, &item.Path
		mode := "copy"
		if boolFlag(command, "link") {
			mode = "link"
		}
		result.Details = map[string]any{"name": item.Name, "type": item.Kind, "path": item.Path, "size": item.Size, "hash": item.Hash, "tags": item.Tags, "added_at": item.AddedAt, "mode": mode}
	case "update":
		if len(args) == 0 && (optionalFlag(command, "set-url") != nil || optionalFlag(command, "channel") != nil) {
			url, channel := "", ""
			if value := optionalFlag(command, "set-url"); value != nil {
				url = *value
			}
			if value := optionalFlag(command, "channel"); value != nil {
				channel = *value
			}
			if err := service.SetPublishURL(project, url, channel, dryRun); err != nil {
				return err
			}
			if application.Quiet {
				return nil
			}
			if application.Format == "json" {
				return output.RenderSuccess(stdout, "json", map[string]any{"url": url, "channel": channel})
			}
			return output.RenderRaw(stdout, "text", fmt.Sprintf("Set publish URL: %s (channel: %s)", url, channel))
		}
		all := boolFlag(command, "all")
		if len(args) > 0 && all {
			return output.Usage("<PATH> and --all are mutually exclusive", "provide either <PATH> or --all, not both")
		}
		if len(args) == 0 && !all {
			return output.Usage("provide <PATH> or --all", "provide a path to update a single asset, or --all to update all")
		}
		if all {
			items, err := service.UpdateAll(project, dryRun)
			if err != nil {
				return err
			}
			return renderAssetUpdates(stdout, &view, items, dryRun)
		}
		report, err := service.UpdateOne(project, application.CWD, args[0], dryRun)
		if err != nil {
			return err
		}
		result.Verb, result.Identity, result.Path = output.Update, report.Path, &report.Path
		changes := map[string]any{}
		if report.Changed {
			changes["size"] = map[string]any{"from": report.OldSize, "to": report.NewSize}
			changes["hash"] = map[string]any{"from": report.OldHash, "to": report.NewHash}
		}
		result.Details = map[string]any{"changes": changes, "changed": report.Changed}
	case "index":
		report, err := service.Reconcile(project, dryRun, boolFlag(command, "refresh-metadata"))
		if err != nil {
			return err
		}
		count := uint64(len(report.Added)+len(report.Removed)) + report.KeptCount
		result.Verb = output.Index
		result.Details = map[string]any{"added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": count, "refreshed": report.Refreshed}
		if application.Format == "json" {
			if application.Quiet {
				return nil
			}
			return output.RenderSuccess(stdout, "json", map[string]any{"kind": "asset", "added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": count, "dry_run": dryRun, "details": map[string]any{"refreshed": report.Refreshed}})
		}
	case "clean":
		report, err := service.Clean(project, dryRun)
		if err != nil {
			return err
		}
		if application.Quiet {
			return nil
		}
		if application.Format == "json" {
			return output.RenderSuccess(stdout, "json", report)
		}
		prefix := ""
		if dryRun {
			prefix = "[dry-run] "
		}
		text := prefix + "No stale entries found."
		if report.RemovedCount > 0 {
			lines := []string{fmt.Sprintf("%sCleaned %d stale entries from index.", prefix, report.RemovedCount)}
			for _, path := range report.StaleEntries {
				lines = append(lines, "  removed: "+path)
			}
			text = strings.Join(lines, "\n")
		}
		return output.RenderRaw(stdout, "text", text)
	case "rename":
		for _, path := range args {
			if err := content.ValidateEntityPath(project, path); err != nil {
				return err
			}
		}
		result.Verb, result.Identity, result.OldIdentity = output.Rename, args[1], &args[0]
		if !dryRun {
			if err := service.Rename(project, args[0], args[1], boolFlag(command, "force")); err != nil {
				return err
			}
			result.Path = &args[1]
		}
	case "move":
		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		target, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "to-project"), application.CWD, false)
		if err != nil {
			return err
		}
		report, err := service.Move(project, target, args[0], dryRun)
		if err != nil {
			return err
		}
		result.Verb, result.Identity, result.OldIdentity = output.Move, report.NewPath, &report.OldPath
		if !dryRun {
			result.Path = &report.NewPath
		}
		result.Details = map[string]any{"old_path": report.OldPath, "new_path": report.NewPath, "from_scope": project, "to_scope": target}
		view.RepoRoot = application.RepoRoot
	case "remove":
		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "asset", Identity: args[0], Yes: boolFlag(command, "yes"), Force: boolFlag(command, "force")}, stderr, nil); err != nil {
			return err
		}
		if _, err := service.Remove(project, args[0], boolFlag(command, "force"), dryRun); err != nil {
			return err
		}
		result.Verb, result.Identity = output.Remove, args[0]
		result.Details["removed"] = true
	}
	return renderProjectVerb(stdout, &view, result)
}

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

func assetReadCommand(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project := application.RepoRoot
	if command.Name() == "show" {
		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		item, err := asset.Show(project, args[0])
		if err != nil {
			return err
		}
		if application.Quiet {
			return nil
		}
		block := output.ShowBlock{Kind: "asset", Identity: item.Path, Fields: []output.ShowField{{Label: "Name", Value: output.ShowValue{Text: item.Name}}, {Label: "Type", Value: output.ShowValue{Text: item.Kind}}, {Label: "Path", Value: output.ShowValue{Text: item.Path, Path: true}}, {Label: "Size", Value: output.ShowValue{Text: fmt.Sprintf("%d bytes", item.Size)}}}}
		if item.Hash != "" {
			block.Fields = append(block.Fields, output.ShowField{Label: "Hash", Value: output.ShowValue{Text: item.Hash}})
		}
		if len(item.Tags) > 0 {
			block.Fields = append(block.Fields, output.ShowField{Label: "Tags", Value: output.ShowValue{Text: strings.Join(item.Tags, ", ")}})
		}
		block.Fields = append(block.Fields, output.ShowField{Label: "Added", Value: output.ShowValue{Text: item.AddedAt}})
		if application.Format == "json" {
			extra, err := jsonObject(item)
			if err != nil {
				return err
			}
			return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, extra))
		}
		profile := commandProfile(stdout, application)
		return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: project, Hyperlinks: profile.Hyperlinks}))
	}
	items, err := asset.List(project, optionalFlag(command, "filter"), optionalFlag(command, "type"))
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		objects := make([]map[string]any, 0, len(items))
		for _, item := range items {
			object, err := jsonObject(item)
			if err != nil {
				return err
			}
			object["identity"] = item.Path
			objects = append(objects, object)
		}
		return output.RenderSuccess(stdout, "json", output.JSONCollection("assets", objects))
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "PATH", "SIZE"}, Plural: "assets"}
	for _, item := range items {
		row := output.TextCells(item.Name, item.Kind, item.Path, fmt.Sprint(item.Size))
		row[2].Path = true
		view.Rows = append(view.Rows, row)
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = project
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}
