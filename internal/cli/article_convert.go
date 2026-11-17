package cli

import (
	"fmt"
	"io"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
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
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return err
	}
	paths := []string{}
	if index.Value.Articles != nil {
		for _, article := range *index.Value.Articles {
			paths = append(paths, article.Path)
		}
	}
	if selector := optionalFlag(command, "article"); selector != nil {
		path, err := content.ResolveArticleSelector(project, *selector)
		if err != nil {
			return err
		}
		paths = []string{path}
	}
	direction, source := "", "explicit"
	if boolFlag(command, "to-single-file") {
		direction = "to_single_file"
	} else if boolFlag(command, "to-directory") {
		direction = "to_directory"
	} else {
		choices := []string{}
		counts := map[string]int{}
		for _, candidate := range []string{"to_single_file", "to_directory"} {
			items, err := content.InspectArticleConversions(root, project, paths, candidate, false)
			if err != nil {
				return err
			}
			for _, item := range items {
				if item.Eligible {
					counts[candidate]++
				}
			}
			if counts[candidate] > 0 {
				choices = append(choices, candidate)
			}
		}
		if len(choices) == 0 {
			return output.Usage("no eligible articles found for conversion", "verify that the project has articles that can be converted")
		}
		if len(choices) > 1 {
			return output.Usage("ambiguous conversion direction; both --to-single-file and --to-directory are possible", "pass --to-single-file or --to-directory to specify the desired direction")
		}
		direction, source = choices[0], "inferred"
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
	inspections, err := content.InspectArticleConversions(root, project, paths, direction, merge)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	summary := model.ConversionSummary{Kind: "article", Direction: direction, DirectionSource: source, DryRun: dryRun, ScannedCount: uint64(len(inspections)), Converted: model.JSONArray[model.ConversionResult]{}, Skipped: model.JSONArray[model.ConversionResult]{}, Failed: model.JSONArray[model.ConversionResult]{}}
	for _, inspection := range inspections {
		if !inspection.Eligible {
			summary.Skipped = append(summary.Skipped, inspection.Result("skipped", false, false))
			continue
		}
		if dryRun {
			summary.Converted = append(summary.Converted, inspection.Result("would_convert", false, false))
			continue
		}
		mutation, err := content.PlanArticleConversion(root, project, inspection, time.Time{})
		if err == nil {
			err = applyContentMutation(root, mutation)
		}
		if err != nil {
			result := inspection.Result("failed", false, false)
			reason := err.Error()
			result.Reason = &reason
			summary.Failed = append(summary.Failed, result)
		} else {
			summary.Converted = append(summary.Converted, inspection.Result("converted", true, true))
		}
	}
	summary.ConvertedCount, summary.SkippedCount, summary.FailedCount = uint64(len(summary.Converted)), uint64(len(summary.Skipped)), uint64(len(summary.Failed))
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
