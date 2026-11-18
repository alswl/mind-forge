package main

import (
	"fmt"
	"io"
	"os"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func optionalFlag(command *cobra.Command, name string) *string {
	if !flagPresent(command, name) {
		return nil
	}
	value := stringFlag(command, name)
	return &value
}

func commandProfile(stdout io.Writer, application *app.Context) output.TerminalProfile {
	profile := output.BuildTerminalProfile(output.CapabilityInput{Width: 200})
	if file, ok := stdout.(*os.File); ok {
		profile = output.ProbeTerminal(file)
	}
	if !application.Color {
		profile.Styling = false
		profile.Hyperlinks = false
		profile.ColorMode = "none"
		profile.Truecolor = false
	}
	return profile
}

func renderProjectVerb(stdout io.Writer, application *app.Context, result output.VerbResult) error {
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", output.VerbEnvelope(result))
	}
	profile := commandProfile(stdout, application)
	return output.RenderRaw(stdout, "text", output.RenderVerb(result, output.DisplayOptions{RepoRoot: application.RepoRoot, Hyperlinks: profile.Hyperlinks}))
}

func listProjects(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	entries, warnings, err := content.ListProjects(application.RepoRoot)
	if err != nil {
		return err
	}
	if err := articleWarnings(stderr, warnings); err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		items := make([]map[string]any, 0, len(entries))
		for _, entry := range entries {
			path := strings.TrimPrefix(entry.Path, "./")
			items = append(items, map[string]any{"identity": entry.Name, "name": entry.Name, "path": path, "created_at": entry.CreatedAt, "archived_at": entry.ArchivedAt, "document_count": entry.DocumentCount, "last_activity_at": entry.LastActivityAt})
		}
		return output.RenderSuccess(stdout, "json", output.JSONCollection("projects", items))
	}
	profile := commandProfile(stdout, application)
	options := output.ListOptionsFromProfile(profile, boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = application.RepoRoot
	view := output.List{Headers: []string{"PATH", "NAME", "ITEMS", "STATUS", "UPDATED", "CREATED"}, Plural: "projects"}
	date := func(value string) string {
		parsed, err := time.Parse(time.RFC3339, value)
		if err != nil {
			return "-"
		}
		return parsed.UTC().Format("2006-01-02 15:04")
	}
	for _, entry := range entries {
		status, color := "active", "\x1b[32m"
		if entry.ArchivedAt != nil {
			status, color = "archived", "\x1b[2m"
		}
		updated := ""
		if entry.LastActivityAt != nil {
			updated = *entry.LastActivityAt
		}
		cells := output.TextCells(strings.TrimPrefix(entry.Path, "./"), entry.Name, fmt.Sprint(entry.DocumentCount), status, date(updated), date(entry.CreatedAt))
		cells[0].Path = true
		cells[3].ANSIPrefix = color
		view.Rows = append(view.Rows, cells)
	}
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}

func showProject(application *app.Context, selector string, stdout io.Writer) error {
	inspection, err := content.InspectProject(application.RepoRoot, application.CWD, selector)
	if err != nil {
		return err
	}
	if inspection.Mind != nil {
		details := inspection.Mind
		identity, minds, count := details.Identity, details.Minds, details.OutputCount
		data := map[string]any{"kind": "project", "identity": identity, "project_identity": identity, "content_layout": details.Layout, "mind_count": len(minds), "output_count": count, "minds": minds}
		if application.Quiet {
			return nil
		}
		if application.Format == "json" {
			return output.RenderSuccess(stdout, "json", data)
		}
		return output.RenderRaw(stdout, "text", fmt.Sprintf("Project:  %s\nLayout:  %s\nMinds:  %d\nOutputs:  %d", identity, details.Layout, len(minds), count))
	}
	details := inspection.Legacy
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", map[string]any{"kind": "project", "identity": details.Name, "name": details.Name, "path": details.Path, "article_count": details.ArticleCount, "source_count": details.SourceCount, "asset_count": details.AssetCount, "last_active": details.LastActive, "mind_yaml_summary": details.MindYamlSummary, "layout": details.Layout})
	}
	text := func(label, value string) output.ShowField {
		return output.ShowField{Label: label, Value: output.ShowValue{Text: value}}
	}
	fields := []output.ShowField{text("Name", details.Name), {Label: "Path", Value: output.ShowValue{Text: details.Path, Path: true}}, text("Articles", fmt.Sprint(details.ArticleCount)), text("Sources", fmt.Sprint(details.SourceCount)), text("Assets", fmt.Sprint(details.AssetCount)), {Label: "Last active", Value: output.ShowValue{Optional: true, Present: details.LastActive != nil}}}
	if details.LastActive != nil {
		fields[5].Value.Text = *details.LastActive
	}
	if summary := details.MindYamlSummary; summary != nil {
		types := strings.Join(summary.Types, ", ")
		if types == "" {
			types = "-"
		}
		fields = append(fields, text("Schema", "v"+summary.SchemaVersion+" (types: "+types+")"), text("Source dirs", strings.Join(summary.SourceDirs, ", ")), text("Assets dir", summary.AssetsDir))
	}
	block := output.ShowBlock{Kind: "project", Identity: details.Name, Fields: fields}
	if layout := details.Layout; layout != nil {
		block.Sections = []output.ShowSection{{Heading: "Layout", Fields: []output.ShowField{text("Articles", layout.Articles), text("Sources", layout.Sources), text("Assets", layout.Assets), text("Templates", layout.Templates), text("Build output", layout.BuildOutput)}}}
	}
	return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: application.RepoRoot, Hyperlinks: commandProfile(stdout, application).Hyperlinks}))
}
