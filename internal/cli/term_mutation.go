package cli

import (
	"io"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/term"
	"github.com/spf13/cobra"
)

func termMutationCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	dry, force := boolFlag(c, "dry-run"), boolFlag(c, "force")
	result := output.VerbResult{Kind: "term", Identity: args[0], DryRun: dry, Details: map[string]any{}}
	switch c.Name() {
	case "rename":
		result.Verb = output.Rename
		result.Identity = args[1]
		result.OldIdentity = &args[0]
		result.Details["keep_alias"] = boolFlag(c, "keep-alias")
		if dry {
			return termVerb(a, stdout, stderr, result, nil)
		}
		_, _, warnings, err := termFallback(c, a, args[0], func(s term.Scope) (bool, error) {
			return true, s.Rename(args[0], args[1], boolFlag(c, "keep-alias"), force)
		})
		if err != nil {
			return err
		}
		return termVerb(a, stdout, stderr, result, warnings)
	case "remove":
		if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "term", Identity: args[0], Yes: boolFlag(c, "yes"), Force: force}, stderr, nil); err != nil {
			return err
		}
		_, _, warnings, err := termFallback(c, a, args[0], func(s term.Scope) (bool, error) { return true, s.Remove(args[0], force, dry) })
		if err != nil {
			return err
		}
		result.Verb = output.Remove
		result.Details["removed"] = true
		return termVerb(a, stdout, stderr, result, warnings)
	case "move":
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
		// Without --from-global the legacy move selects a project store, even
		// when its path falls back to the repository root.
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
		if _, err := lifecycleWithHeldLock().Apply(a.RepoRoot, plan.Plan); err != nil {
			return err
		}
		result.Details = map[string]any{"from_scope": plan.Report.FromScope, "to_scope": plan.Report.ToScope, "side_effects": plan.Report.SideEffects}
		return termVerb(a, stdout, stderr, result, nil)
	}
	return nil
}
