package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article new", "article", newArticleNewCommand, func(e commandExecution) error {
		return executeArticleNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeArticleNew(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project
	{

		tags, _ := command.Flags().GetStringArray("tag")
		options := content.ArticleCreateOptions{Slug: optionalFlag(command, "slug"), Template: stringFlag(command, "template"), File: boolFlag(command, "file"), Tags: tags, Draft: boolFlag(command, "draft"), Force: boolFlag(command, "force"), DryRun: dryRun}
		report, err := content.CreateArticle(project, args[0], options)
		if err != nil {
			return err
		}
		path := report.DocsDir + "/" + report.Filename + "/"
		if report.Shape == "file" {
			path = report.DocsDir + "/" + report.Filename + ".md"
		}
		result := output.VerbResult{Verb: output.Create, Kind: "article", Identity: path, DryRun: dryRun, Details: map[string]any{"title": args[0], "filename": report.Filename, "draft": options.Draft, "template": report.Template, "shape": report.Shape, "path": path, "files": report.Files, "typora_front_matter_injected": report.TyporaFrontMatterInjected, "typora_copy_images_to": report.TyporaCopyImagesTo}}
		if !dryRun {
			result.Path = &path
		}
		warnings := []string{}
		if options.Slug == nil && strings.IndexFunc(report.Filename, func(r rune) bool { return r > 127 }) >= 0 {
			warning := "article title produced a non-ASCII filename; consider an English --slug"
			if _, err := fmt.Fprintln(stderr, "warning: "+warning); err != nil {
				return err
			}
			warnings = append(warnings, warning)
		}
		if application.Quiet {
			return nil
		}
		if application.Format == "json" {
			envelope := output.VerbEnvelope(result)
			if len(warnings) > 0 {
				envelope["warnings"] = warnings
			}
			return output.RenderSuccess(stdout, "json", envelope)
		}
		return renderProjectVerb(stdout, &view, result)
	}
}
