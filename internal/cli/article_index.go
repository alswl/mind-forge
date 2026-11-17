package cli

import (
	"io"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func articleIndexCommand(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project
	if command.Name() == "index" {
		report, warnings, err := content.IndexArticles(project, dryRun, time.Time{})
		if err != nil {
			return err
		}
		if err := articleWarnings(stderr, warnings); err != nil {
			return err
		}
		return renderProjectVerb(stdout, &view, report)
	}
	issues, err := content.LintArticles(project, boolFlag(command, "fix") && !dryRun)
	if err != nil {
		return err
	}
	rules, _ := command.Flags().GetStringArray("rule")
	severity := severityRank(stringFlag(command, "severity"))
	objects := []any{}
	errors, warnings, info := 0, 0, 0
	for _, issue := range issues {
		included := len(rules) == 0
		for _, rule := range rules {
			if rule == issue.Kind {
				included = true
				break
			}
		}
		if !included || severityRank(issue.Severity) > severity {
			continue
		}
		switch issue.Severity {
		case "error":
			errors++
		case "warning":
			warnings++
		case "info":
			info++
		}
		object, err := jsonObject(issue)
		if err != nil {
			return err
		}
		objects = append(objects, object)
	}
	summary := map[string]any{"errors": errors, "warnings": warnings, "info": info, "fixed": 0}
	if !application.Quiet {
		if application.Format == "json" {
			err = output.RenderSuccess(stdout, "json", map[string]any{"kind": "article", "issues": objects, "summary": summary, "dry_run": dryRun})
		} else {
			report := output.VerbResult{Verb: output.Lint, Kind: "article", DryRun: dryRun, Details: map[string]any{"issues": objects, "summary": summary}}
			err = renderProjectVerb(stdout, &view, report)
		}
		if err != nil {
			return err
		}
	}
	maxWarnings, _ := command.Flags().GetInt64("max-warnings")
	if errors > 0 || flagPresent(command, "max-warnings") && int64(warnings) > maxWarnings {
		return &commandExit{1}
	}
	return nil
}

func severityRank(value string) int {
	switch value {
	case "error":
		return 0
	case "warning":
		return 1
	default:
		return 2
	}
}
