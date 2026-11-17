package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/publish"
	"github.com/alswl/mind-forge/internal/textcase"
	"github.com/spf13/cobra"
)

func publishCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	if c.HasSubCommands() {
		return c.Help()
	}
	if c.Parent().Name() == "target" {
		return publisherCommand(c, a, args, stdout)
	}
	selector := args[0]
	project, err := content.ProjectFor(a.RepoRoot, a.CWD, optionalFlag(c, "project"), &selector)
	if err != nil {
		return err
	}
	if c.Name() == "update" {
		status, url := optionalFlag(c, "status"), optionalFlag(c, "target-url")
		warnings := []string{}
		if status != nil {
			if _, err := fmt.Fprintln(stderr, "[deprecated] --status is deprecated, use --set status=<value> instead"); err != nil {
				return err
			}
		}
		if url != nil {
			if _, err := fmt.Fprintln(stderr, "[deprecated] --target-url is deprecated, use --set url=<value> instead"); err != nil {
				return err
			}
		}
		sets, _ := c.Flags().GetStringArray("set")
		for _, pair := range sets {
			key, value, ok := strings.Cut(pair, "=")
			if !ok {
				continue
			}
			switch key {
			case "status":
				if value == "draft" || value == "published" || value == "archived" {
					copy := value
					status = &copy
				}
			case "url":
				copy := value
				url = &copy
			}
		}
		result, location, err := publish.Update(a.RepoRoot, project, selector, publish.UpdateOptions{Target: stringFlag(c, "target"), Status: status, URL: url, DryRun: boolFlag(c, "dry-run")})
		if err != nil {
			return err
		}
		data, err := jsonObject(result)
		if err != nil {
			return err
		}
		if location != nil {
			data["location"] = location
		}
		urlText, dateText := "null", "null"
		if result.Record.TargetURL != nil {
			urlText = *result.Record.TargetURL
		}
		if result.Record.PublishedAt != nil {
			dateText = *result.Record.PublishedAt
		}
		text := fmt.Sprintf("article      %s\ntarget       %s\naction       %s\nstatus       %s\ntarget_url   %s\npublished_at %s\ndry_run      %t", result.Article, result.TargetName, result.Action, result.Record.Status, urlText, dateText, result.DryRun)
		return renderTerm(a, stdout, stderr, data, text, warnings)
	}
	result, location, warnings, err := publish.Run(a.RepoRoot, project, selector, publish.RunOptions{Target: optionalFlag(c, "target"), DryRun: boolFlag(c, "dry-run"), Force: boolFlag(c, "force"), StripFrontMatter: boolFlag(c, "strip-front-matter"), KeepFrontMatter: boolFlag(c, "keep-front-matter")})
	if err != nil {
		return err
	}
	data, err := jsonObject(result)
	if err != nil {
		return err
	}
	if location != nil {
		data["location"] = location
	}
	text := ""
	if result.Local != nil {
		o := result.Local
		text = fmt.Sprintf("target      %s\ntype        local\narticle     %s\nsource      %s\ndestination %s\nsize        %d bytes\ndry_run     %t", o.TargetName, o.Article, o.Source, o.Destination, o.SizeBytes, o.DryRun)
	} else {
		o := result.YuquePrompt
		envelope, err := jsonObject(o)
		if err != nil {
			return err
		}
		delete(envelope, "prompt")
		raw, err := json.Marshal(envelope)
		if err != nil {
			return err
		}
		text = "### Publish Prompt\n\n" + o.Prompt + "\n\n### Envelope\n\n```json\n" + string(raw) + "\n```"
	}
	return renderTerm(a, stdout, stderr, data, text, warnings)
}

func publisherCommand(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {
	report, err := publish.Discover(a.RepoRoot)
	if err != nil {
		return err
	}
	if c.Name() == "show" {
		for _, item := range report.Publishers {
			p := item.View
			if !textcase.ASCIIEqualFold(p.Name, args[0]) {
				continue
			}
			if a.Quiet {
				return nil
			}
			label, description := "", ""
			if p.Label != nil {
				label = *p.Label
			}
			if p.Description != nil {
				description = *p.Description
			}
			required := "-"
			if len(p.RequiredInputs) > 0 {
				required = strings.Join(p.RequiredInputs, ", ")
			}
			block := output.ShowBlock{Kind: "publish_target", Identity: p.Name, Fields: []output.ShowField{
				{Label: "Name", Value: output.ShowValue{Text: p.Name}}, {Label: "Type", Value: output.ShowValue{Text: p.TargetType}}, {Label: "Status", Value: output.ShowValue{Text: p.Status}},
				{Label: "Label", Value: output.ShowValue{Text: label, Optional: true, Present: p.Label != nil}}, {Label: "Description", Value: output.ShowValue{Text: description, Optional: true, Present: p.Description != nil}},
				{Label: "Source", Value: output.ShowValue{Text: p.SourcePath, Path: true}}, {Label: "Required inputs", Value: output.ShowValue{Text: required}},
			}}
			if a.Format == "json" {
				data, err := jsonObject(p)
				if err != nil {
					return err
				}
				return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, data))
			}
			return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: a.RepoRoot, Hyperlinks: commandProfile(stdout, a).Hyperlinks}))
		}
		return output.Usage("publish target '"+args[0]+"' not found", "use `mf publish target list` to see available targets")
	}
	if a.Quiet {
		return nil
	}
	if a.Format == "json" {
		items := []any{}
		for _, item := range report.Publishers {
			data, err := jsonObject(item.View)
			if err != nil {
				return err
			}
			data["identity"] = item.View.Name
			items = append(items, data)
		}
		return output.RenderSuccess(stdout, "json", map[string]any{"publish_targets": items, "diagnostics": report.Diagnostics})
	}
	var diagnostics strings.Builder
	for _, d := range report.Diagnostics {
		path, name := "-", "-"
		if d.Path != nil {
			path = *d.Path
		}
		if d.PublisherName != nil {
			name = *d.PublisherName
		}
		fmt.Fprintf(&diagnostics, "[%s] %s (%s): %s\n", d.Kind, path, name, d.Message)
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "STATUS"}, Plural: "publish targets"}
	for _, item := range report.Publishers {
		view.Rows = append(view.Rows, output.TextCells(item.View.Name, item.View.TargetType, item.View.Status))
	}
	opts := output.ListOptionsFromProfile(commandProfile(stdout, a), boolFlag(c, "no-headers"), boolFlag(c, "no-trunc"))
	return output.RenderRaw(stdout, "text", diagnostics.String()+output.RenderList(view, opts))
}
