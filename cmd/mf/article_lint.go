package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article lint", "article", newArticleLintCommand, func(e commandExecution) error {
		return executeArticleLint(e.Command, e.Application, e.Stdout, e.Stderr)
	})
}

func newArticleLintCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "lint"}
	return configureCommand(command, item, handler)
}

func executeArticleLint(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	final := stringFlag(command, "mode") == "final"
	if final && boolFlag(command, "fix") {
		return output.Usage("--mode final cannot be combined with --fix", "review the findings and edit the source manually")
	}
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project

	var issues []model.LintIssue
	status := optionalFlag(command, "status")
	if final {
		issues, err = content.LintFinalArticles(command.Context(), project, status)
	} else {
		issues, err = content.LintArticlesRequest(command.Context(), application.RepoRoot, project, boolFlag(command, "fix") && !dryRun, status)
	}
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
