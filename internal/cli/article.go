package cli

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func articleBlockCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	article, err := content.ResolveArticleSelector(project, args[0])
	if err != nil {
		return err
	}
	dryRun, force := boolFlag(command, "dry-run"), boolFlag(command, "force")
	result := output.VerbResult{Kind: "block", DryRun: dryRun}
	switch command.Name() {
	case "rename":
		old := article + "/" + args[1]
		result.Verb, result.Identity, result.OldIdentity, result.Details = output.Rename, article+"/"+args[2], &old, map[string]any{}
		if _, err := content.CheckedPath(project, article); err != nil {
			return err
		}
		if !dryRun {
			report, err := content.RenameBlock(project, article, args[1], args[2], force)
			if err != nil {
				return err
			}
			oldPath, newPath := article+"/"+report.OldFilename, article+"/"+report.NewFilename
			result.Identity, result.OldIdentity, result.Path = newPath, &oldPath, &newPath
			result.Details = map[string]any{"old_filename": report.OldFilename, "new_filename": report.NewFilename, "article_path": report.ArticlePath}
		}
	case "rm":
		if _, err := content.CheckedPath(project, article); err != nil {
			return err
		}
		if !dryRun {
			if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "block", Identity: article + "/" + args[1], Yes: boolFlag(command, "yes"), Force: force}, stderr, nil); err != nil {
				return err
			}
		}
		report, err := content.RemoveBlock(project, article, args[1], dryRun)
		if err != nil {
			return err
		}
		path := report.ArticlePath + "/" + report.RemovedFilename
		result.Verb, result.Identity, result.Path = output.Remove, path, &path
		result.Details = map[string]any{"article_path": report.ArticlePath, "removed_filename": report.RemovedFilename, "remaining_blocks": report.RemainingBlocks}
	default:
		start, _ := command.Flags().GetUint64("start")
		options := content.BlockEditOptions{After: optionalFlag(command, "after"), Start: start, DryRun: dryRun}
		var report content.BlockEditReport
		switch command.Name() {
		case "new":
			report, err = content.NewBlock(project, article, args[1], options)
		case "move":
			report, err = content.MoveBlock(project, article, args[1], options)
		case "renumber":
			report, err = content.RenumberBlocks(project, article, start, dryRun)
		}
		if err != nil {
			return err
		}
		result.Verb, result.Identity, result.Path, result.OldIdentity = output.Update, article, report.NewPath, report.OldPath
		if report.NewPath != nil {
			result.Identity = *report.NewPath
		} else if report.OldPath != nil {
			result.Identity = *report.OldPath
		}
		result.Details = map[string]any{"article_path": report.ArticlePath, "old_path": report.OldPath, "new_path": report.NewPath, "order": report.Order}
	}
	view := *application
	view.RepoRoot = project
	return renderProjectVerb(stdout, &view, result)
}

func articleCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project
	if command.Name() == "new" {
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
	report, err := content.UpdateArticle(project, args[0], content.ArticleUpdateOptions{Title: optionalFlag(command, "title"), Status: optionalFlag(command, "status"), DryRun: dryRun})
	if err != nil {
		return err
	}
	path := report.Article.Path
	result := output.VerbResult{Verb: output.Update, Kind: "article", Identity: path, Path: &path, DryRun: dryRun, Details: map[string]any{"changes": report.Changes, "article": report.Article}}
	return renderProjectVerb(stdout, &view, result)
}
