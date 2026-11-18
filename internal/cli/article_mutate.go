package cli

import (
	"io"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"github.com/spf13/cobra"
)

func validateArticleArgument(project, input string) error {
	if err := store.RejectTraversal(input, "project"); err != nil {
		return err
	}
	if strings.ContainsAny(input, "/\\") {
		target := input
		if !filepath.IsAbs(target) {
			target = filepath.Join(project, input)
		}
		_, err := store.CanonicalWithin(project, target)
		return err
	}
	return nil
}

// Run owns the root write lock for these mutations. The engine still checks
// its journal and inventory, borrowing that lock instead of acquiring it twice.
func applyContentMutation(root string, mutation content.ContentMutation) error {
	_, err := lifecycleWithHeldLock().Apply(root, mutation.Plan)
	return err
}

func lifecycleWithHeldLock() lifecycle.Engine {
	return lifecycle.Engine{AcquireLock: func(string) (io.Closer, error) { return io.NopCloser(strings.NewReader("")), nil }}
}

func articleMutationCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	project, err := app.ResolveProject(root, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun, force := boolFlag(command, "dry-run"), boolFlag(command, "force")
	view := *application
	view.RepoRoot = project
	var mutation content.ContentMutation
	switch command.Name() {
	case "rename":
		if err := validateArticleArgument(project, args[0]); err != nil {
			return err
		}
		if err := validateArticleArgument(project, args[1]); err != nil {
			return err
		}
		if dryRun {
			old := args[0]
			return renderProjectVerb(stdout, &view, output.VerbResult{Verb: output.Rename, Kind: "article", Identity: args[1], OldIdentity: &old, DryRun: true, Details: map[string]any{}})
		}
		mutation, err = content.PlanArticleRename(root, project, args[0], args[1], force, time.Time{})
	case "move":
		target, resolveErr := app.ResolveProject(root, optionalFlag(command, "to-project"), application.CWD, false)
		if resolveErr != nil {
			return resolveErr
		}
		mutation, err = content.PlanArticleMove(root, project, target, args[0], dryRun)
		view.RepoRoot = root
	case "remove":
		if err := validateArticleArgument(project, args[0]); err != nil {
			return err
		}
		if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "article", Identity: args[0], Yes: boolFlag(command, "yes"), Force: force}, stderr, nil); err != nil {
			return err
		}
		mutation, err = content.PlanArticleRemove(root, project, args[0], dryRun)
	}
	if err != nil {
		return err
	}
	if !dryRun {
		if err := applyContentMutation(root, mutation); err != nil {
			return err
		}
	}
	return renderProjectVerb(stdout, &view, mutation.Result)
}
