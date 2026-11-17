package cli

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/source"
	"github.com/spf13/cobra"
)

func sourceCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	if command.Name() == "remove" && len(args) > 0 && !strings.Contains(args[0], "/") && !strings.HasPrefix(args[0], "sources") {
		if err := output.WarnDeprecation(stderr, "positional NAME", "full PATH (e.g., sources/yuque/foo.md)", !application.Color); err != nil {
			return err
		}
	}
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "source", DryRun: dryRun, Details: map[string]any{}}
	switch command.Name() {
	case "list", "show":
		return sourceReadCommand(command, &view, args, stdout)
	case "new":
		kind, err := sourceInputKind(command, args[0])
		if err != nil {
			return err
		}
		options := source.AddOptions{Name: optionalFlag(command, "name"), Kind: kind, SourceKind: optionalFlag(command, "source-kind"), Link: boolFlag(command, "link"), Force: boolFlag(command, "force"), RegisterOnly: boolFlag(command, "register-only"), DryRun: dryRun}
		if dryRun && !options.RegisterOnly {
			name, err := source.DeriveNameFromPath(args[0])
			if err != nil {
				name = "unknown"
			}
			if options.Name != nil {
				name = *options.Name
			}
			result.Verb, result.Identity, result.Path = output.Add, name, &name
			result.Details = map[string]any{"input": args[0]}
			return renderProjectVerb(stdout, &view, result)
		}
		outcome, err := service.Add(command.Context(), application.RepoRoot, project, application.CWD, args[0], options)
		if err != nil {
			return err
		}
		item := outcome.Source
		result.Verb, result.Identity, result.Path = output.Add, item.Name, item.Path
		result.Details = map[string]any{"name": item.Name, "type": item.Kind, "url": item.URL, "path": item.Path, "added_at": item.AddedAt, "updated_at": item.UpdatedAt, "mode": outcome.Mode, "replaced": outcome.Replaced}
		if outcome.ReplacedLocation != nil {
			result.Details["replaced_location"] = *outcome.ReplacedLocation
		}
		if !application.Quiet && application.Format != "json" && outcome.ReplacedLocation != nil {
			profile := commandProfile(stdout, application)
			text := output.RenderVerb(result, output.DisplayOptions{RepoRoot: project, Hyperlinks: profile.Hyperlinks}) + "\n  replaced registration at " + *outcome.ReplacedLocation
			return output.RenderRaw(stdout, "text", text)
		}
	case "update", "rename", "move", "remove":
		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		if command.Name() == "remove" {
			if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "source", Identity: args[0], Yes: boolFlag(command, "yes"), Force: boolFlag(command, "force")}, stderr, nil); err != nil {
				return err
			}
			report, err := service.Remove(project, args[0], boolFlag(command, "keep-file"), boolFlag(command, "force"), dryRun)
			if err != nil {
				return err
			}
			result.Verb, result.Identity, result.Path = output.Remove, report.Source.Name, report.Source.Path
			result.Details["removed"] = true
			break
		}
		items, err := source.List(project, nil, nil)
		if err != nil {
			return err
		}
		name, err := source.ResolveSelector(items, args[0])
		if err != nil {
			return err
		}
		if command.Name() == "update" {
			rename, url := optionalFlag(command, "rename"), optionalFlag(command, "url")
			changes := map[string]any{}
			if rename != nil {
				changes["rename"] = map[string]any{"from": args[0], "to": *rename}
			}
			if url != nil {
				changes["url"] = map[string]any{"to": *url}
			}
			result.Verb, result.Identity = output.Update, name
			result.Details["changes"] = changes
			if rename != nil {
				result.OldIdentity = &name
				result.Identity = *rename
			}
			if !dryRun {
				item, err := service.Update(project, name, rename, url)
				if err != nil {
					return err
				}
				result.Details["source"] = item
			}
		} else if command.Name() == "rename" {
			report, err := service.Rename(project, application.CWD, name, args[1], boolFlag(command, "force"), dryRun)
			if err != nil {
				return err
			}
			result.Verb, result.Identity, result.OldIdentity, result.Path = output.Rename, report.After.Name, &report.Before.Name, report.After.Path
		} else {
			target, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "to-project"), application.CWD, false)
			if err != nil {
				return err
			}
			report, err := service.Move(project, target, name, dryRun)
			if err != nil {
				return err
			}
			result.Verb, result.Identity, result.OldIdentity, result.Path = output.Move, report.Name, &report.OldPath, &report.NewPath
			result.Details = map[string]any{"name": report.Name, "old_path": report.OldPath, "new_path": report.NewPath, "dry_run": report.DryRun}
			view.RepoRoot = application.RepoRoot
		}
	case "index", "clean":
		var report model.SourceIndexReport
		if command.Name() == "index" {
			report, err = service.Reconcile(project, dryRun)
		} else {
			report, err = service.Clean(project, dryRun)
		}
		if err != nil {
			return err
		}
		if command.Name() == "clean" {
			if application.Quiet {
				return nil
			}
			if application.Format == "json" {
				return output.RenderSuccess(stdout, "json", report)
			}
			text := "No dirty sources."
			if len(report.Removed) > 0 {
				prefix := ""
				if dryRun {
					prefix = "[dry-run] "
				}
				lines := []string{}
				for _, entry := range report.Removed {
					lines = append(lines, fmt.Sprintf("%s- removed: %s (%s)", prefix, entry.Name, entry.Kind))
				}
				lines = append(lines, fmt.Sprintf("%skept: %d entries", prefix, report.KeptCount))
				text = strings.Join(lines, "\n")
			}
			return output.RenderRaw(stdout, "text", text)
		}
		result.Verb = output.Index
		result.Details = map[string]any{"added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": uint64(len(report.Added)+len(report.Removed)) + report.KeptCount}
		if application.Format == "json" {
			if application.Quiet {
				return nil
			}
			data := result.Details
			data["kind"], data["dry_run"] = "source", dryRun
			return output.RenderSuccess(stdout, "json", data)
		}
	}
	return renderProjectVerb(stdout, &view, result)
}

func sourceInputKind(command *cobra.Command, input string) (*string, error) {
	form := source.ClassifyInput(input)
	kind := optionalFlag(command, "file-kind")
	if kind == nil && optionalFlag(command, "source-kind") == nil {
		return nil, nil
	}
	value := "file"
	if form == source.URLInput {
		value = "web"
	}
	if kind != nil && *kind != "auto" {
		value = *kind
	}
	if form == source.URLInput && (value == "pdf" || value == "file") {
		return nil, output.Usage("cannot use --type pdf or --type file with a URL input", "download the file first, then add the local path")
	}
	if form == source.PathInput && (value == "rss" || value == "web") {
		return nil, output.Usage("cannot use --type rss or --type web with a local file input", "pass an http(s):// URL")
	}
	return &value, nil
}

func sourceReadCommand(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project := application.RepoRoot
	kind := optionalFlag(command, "type")
	if kind != nil && *kind == "auto" {
		return output.Usage("--type auto is not valid for listing; specify a concrete type", "use --type pdf, file, rss, or web")
	}
	items, err := source.List(project, optionalFlag(command, "filter"), kind)
	if err != nil {
		return err
	}
	if command.Name() == "show" {
		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		name, err := source.ResolveSelector(items, args[0])
		if err != nil {
			return err
		}
		for _, item := range items {
			if item.Name == name {
				if application.Quiet {
					return nil
				}
				typeName := item.Kind
				if item.SourceKind != nil {
					typeName += " (" + *item.SourceKind + ")"
				}
				block := output.ShowBlock{Kind: "source", Identity: item.Name, Fields: []output.ShowField{{Label: "Name", Value: output.ShowValue{Text: item.Name}}, {Label: "Type", Value: output.ShowValue{Text: typeName}}, {Label: "Location", Value: output.ShowValue{Text: sourceLocation(item), Path: true}}, {Label: "Added", Value: output.ShowValue{Text: item.AddedAt}}}}
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
		}
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
			object["identity"] = item.Name
			objects = append(objects, object)
		}
		return output.RenderSuccess(stdout, "json", map[string]any{"sources": objects})
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "LOCATION"}, Plural: "sources"}
	for _, item := range items {
		row := output.TextCells(item.Name, item.Kind, sourceLocation(item))
		row[2].Path = true
		view.Rows = append(view.Rows, row)
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = project
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}

func sourceLocation(item model.Source) string {
	if item.Path != nil {
		return *item.Path
	}
	if item.URL != nil {
		return *item.URL
	}
	return "-"
}
