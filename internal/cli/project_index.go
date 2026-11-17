package cli

import (
	"io"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
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
	rules, err := content.NormalizeProjectLintRules(rules)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	fix := boolFlag(command, "fix") && !dryRun
	objects := []map[string]any{}
	summary := content.ProjectLintSummary{}
	if selector := optionalFlag(command, "project"); selector != nil {
		project, err := app.ResolveProject(application.RepoRoot, selector, application.CWD, false)
		if err != nil {
			return err
		}
		issues, counts, err := content.LintProject(project, rules, fix)
		if err != nil {
			return err
		}
		summary = counts
		for _, issue := range issues {
			object, err := jsonObject(issue)
			if err != nil {
				return err
			}
			objects = append(objects, object)
		}
	} else {
		objects, summary, err = content.LintRepository(application.RepoRoot, rules, fix)
		if err != nil {
			return err
		}
	}
	if flagPresent(command, "severity") {
		rank := severityRank(stringFlag(command, "severity"))
		filtered := []map[string]any{}
		summary.Errors, summary.Warnings = 0, 0
		for _, issue := range objects {
			severity, _ := issue["severity"].(string)
			if severityRank(severity) > rank {
				continue
			}
			filtered = append(filtered, issue)
			// The frozen explicit severity filter counts all visible issues, including fixed ones.
			if severity == "error" {
				summary.Errors++
			} else if severity == "warning" {
				summary.Warnings++
			}
		}
		objects = filtered
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
