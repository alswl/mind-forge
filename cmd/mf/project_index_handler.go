package main

import (
	"io"
	"time"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func projectIndexCommand(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	if root == "" {
		root = application.CWD
	}
	result, warnings, err := content.IndexProjects(root, boolFlag(command, "dry-run"), time.Time{})
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
		data := output.VerbEnvelope(result)
		if len(warnings) > 0 {
			data["warnings"] = warnings
		}
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderProjectVerb(stdout, application, result)
}

func projectLintCommand(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	rules, _ := command.Flags().GetStringArray("rule")
	dryRun := boolFlag(command, "dry-run")
	objects, summary, err := content.LintProjectRequest(application.RepoRoot, application.CWD, optionalFlag(command, "project"), rules, boolFlag(command, "fix") && !dryRun, optionalFlag(command, "severity"))
	if err != nil {
		return err
	}

	if !application.Quiet {
		if application.Format == "json" {
			err = output.RenderSuccess(stdout, "json", map[string]any{"kind": "project", "issues": objects, "summary": summary, "dry_run": dryRun})
		} else {
			err = renderProjectVerb(stdout, application, projectLintResult(objects, summary, dryRun))
		}
		if err != nil {
			return err
		}
	}
	maxWarnings, _ := command.Flags().GetInt64("max-warnings")
	if summary.Errors > 0 || flagPresent(command, "max-warnings") && int64(summary.Warnings) > maxWarnings {
		return &commandExit{1}
	}
	return nil
}

func projectLintResult(issues []map[string]any, summary content.ProjectLintSummary, dryRun bool) output.VerbResult {
	values := make([]any, len(issues))
	for i, issue := range issues {
		values[i] = issue
	}
	counts := map[string]any{"errors": summary.Errors, "warnings": summary.Warnings, "fixed": summary.Fixed, "unfixed": summary.Unfixed}
	return output.VerbResult{Verb: output.Lint, Kind: "project", DryRun: dryRun, Details: map[string]any{"issues": values, "summary": counts}}
}
