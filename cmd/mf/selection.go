package main

import (
	"context"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

type ResolvedSelection struct {
	OriginalProjectPresent  bool
	EffectiveProjectPresent bool
	ProjectOrigin           string
	Project                 string
	Args                    []string
}

type selectionContextKey struct{}

func resolveSelection(application *app.Context, item CommandSpec, args []string, project string, present bool) ResolvedSelection {
	selection := ResolvedSelection{OriginalProjectPresent: present, EffectiveProjectPresent: present, Project: project, ProjectOrigin: "none", Args: append([]string(nil), args...)}
	if present {
		selection.ProjectOrigin = "explicit"
	}
	if len(args) > 0 && hasPathSelector(item.Path) {
		if selected, relative, adopted := app.SelectProjectFromPath(application.RepoRoot, application.CWD, args[0], present); adopted {
			selection.Project = selected
			selection.Args[0] = relative
			selection.EffectiveProjectPresent = true
			selection.ProjectOrigin = "auto_path"
		}
	}
	return selection
}

func selectionFromCommand(command *cobra.Command) (ResolvedSelection, bool) {
	if command.Context() == nil {
		return ResolvedSelection{}, false
	}
	selection, ok := command.Context().Value(selectionContextKey{}).(ResolvedSelection)
	return selection, ok
}

// The command adapter exposes effective values only after parsing has finished.
func withSelection(ctx context.Context, selection ResolvedSelection) context.Context {
	return context.WithValue(ctx, selectionContextKey{}, selection)
}
