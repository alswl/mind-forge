package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func articleConvertCommand(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	merge := boolFlag(command, "merge")
	if merge && !boolFlag(command, "to-single-file") {
		return output.Usage("--merge requires --to-single-file", "pass `--to-single-file --merge` to merge multi-block directory articles")
	}
	root := application.RepoRoot
	project, err := app.ResolveProject(root, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	paths, err := content.ArticleConversionPaths(project, optionalFlag(command, "article"))
	if err != nil {
		return err
	}

	direction, source := "", "explicit"
	if boolFlag(command, "to-single-file") {
		direction = "to_single_file"
	} else if boolFlag(command, "to-directory") {
		direction = "to_directory"
	} else {
		inferred, counts, err := content.InferArticleConversion(root, project, paths)
		if err != nil {
			return err
		}
		direction, source = inferred, "inferred"

		flag := "--" + strings.ReplaceAll(direction, "_", "-")
		plural := "s"
		if counts[direction] == 1 {
			plural = ""
		}
		prompt := fmt.Sprintf("No conversion direction specified.\nSuggested direction: %s (%d article%s can be converted)\nProceed? [y/N]: ", flag, counts[direction], plural)
		switch output.PromptConfirmation(prompt, stderr, nil) {
		case output.NotTTY:
			return output.Usage("no conversion direction specified; eligible direction is "+flag, "pass "+flag+" or run in a terminal for interactive confirmation")
		case output.Aborted:
			if application.Quiet {
				return nil
			}
			return output.RenderRaw(stdout, application.Format, "conversion declined")
		}
	}
	summary, err := content.ConvertArticleRequest(command.Context(), root, project, paths, direction, source, merge, boolFlag(command, "dry-run"))
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", summary)
	}
	return output.RenderRaw(stdout, "text", renderArticleConversion(summary))
}

func renderArticleConversion(summary model.ConversionSummary) string {
	prefix, verb := "", "converted"
	if summary.DryRun {
		prefix, verb = "[dry-run] ", "would convert"
	}
	lines := []string{}
	for _, item := range summary.Converted {
		lines = append(lines, fmt.Sprintf("%s%s article: %s -> %s", prefix, verb, item.SourcePath, item.TargetPath))
	}
	for _, item := range summary.Skipped {
		reason := "unknown"
		if item.Reason != nil {
			reason = *item.Reason
		}
		lines = append(lines, fmt.Sprintf("skipped article: %s (%s)", item.SourcePath, reason))
	}
	for _, item := range summary.Failed {
		reason := "unknown error"
		if item.Reason != nil {
			reason = *item.Reason
		}
		lines = append(lines, fmt.Sprintf("failed article: %s (%s)", item.SourcePath, reason))
	}
	lines = append(lines, fmt.Sprintf("%sarticle convert %s: %d %s, %d skipped, %d failed", prefix, "--"+strings.ReplaceAll(summary.Direction, "_", "-"), summary.ConvertedCount, verb, summary.SkippedCount, summary.FailedCount))
	return strings.Join(lines, "\n")
}
