package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term move", "term", newTermMoveCommand, func(e commandExecution) error {
		return executeTermMove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermMoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "move"}
	return configureCommand(command, item, handler)
}

func executeTermMove(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermMoveDetails(c, a, args, stdout, stderr)
}

func executeTermMoveDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	dry, force := boolFlag(c, "dry-run"), boolFlag(c, "force")
	result := output.VerbResult{Kind: "term", Identity: args[0], DryRun: dry, Details: map[string]any{}}

	toGlobal, fromGlobal := boolFlag(c, "to-global"), boolFlag(c, "from-global")
	if !toGlobal && optionalFlag(c, "to-project") == nil {
		return output.Usage("must specify --to-global or --to-project <PROJECT> for the destination", "")
	}
	if fromGlobal && toGlobal {
		return output.Usage("source and destination are both global; nothing to do", "")
	}
	from := term.Scope{Root: a.RepoRoot, Kind: "global"}
	if !fromGlobal {
		var err error
		from, err = termScope(c, a)
		if err != nil {
			return err
		}
	}

	if !fromGlobal {
		from.Kind = "project"
	}
	to := term.Scope{Root: a.RepoRoot, Kind: "global"}
	if !toGlobal {
		root, err := app.ResolveProject(a.RepoRoot, optionalFlag(c, "to-project"), a.CWD, false)
		if err != nil {
			return err
		}
		to = term.Scope{Root: root, Kind: "project"}
	}
	result.Verb = output.Move
	result.Details = map[string]any{"from_scope": from.Kind, "to_scope": to.Kind, "force": force}
	if dry {
		return termVerb(a, stdout, stderr, result, nil)
	}
	plan, err := term.PlanMove(a.RepoRoot, from, to, args[0], force)
	if err != nil {
		return err
	}
	if err := content.ApplyMutation(c.Context(), a.RepoRoot, content.ContentMutation{Plan: plan.Plan}); err != nil {
		return err
	}
	result.Details = map[string]any{"from_scope": plan.Report.FromScope, "to_scope": plan.Report.ToScope, "side_effects": plan.Report.SideEffects}
	return termVerb(a, stdout, stderr, result, nil)
}
