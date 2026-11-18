package main

import (
	"context"
	"os"
	"strings"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

type runtimeContextKey struct{}

func runtimeFromCommand(command *cobra.Command) (app.RuntimeSettings, bool) {
	if command.Context() == nil {
		return app.RuntimeSettings{}, false
	}
	s, ok := command.Context().Value(runtimeContextKey{}).(app.RuntimeSettings)
	return s, ok
}

func runtimeInput(command *cobra.Command, cwd string, env map[string]string) app.RuntimeInput {
	cli := map[string]string{}
	for _, key := range []string{"root", "config", "project", "output", "quiet", "verbose", "no-color"} {
		if flagPresent(command, key) {
			cli[key] = command.Flags().Lookup(key).Value.String()
		}
	}
	return app.RuntimeInput{CWD: cwd, Env: env, CLI: cli, UserConfigDir: os.UserConfigDir}
}

func snapshotEnvironment() map[string]string {
	values := map[string]string{}
	for _, entry := range os.Environ() {
		key, value, ok := strings.Cut(entry, "=")
		if ok {
			values[key] = value
		}
	}
	return values
}

func withRuntime(ctx context.Context, s app.RuntimeSettings) context.Context {
	return context.WithValue(ctx, runtimeContextKey{}, s)
}

func runtimeSelection(application *app.Context, item CommandSpec, args []string, s app.RuntimeSettings, original bool) ResolvedSelection {
	selection := resolveSelection(application, item, args, s.Project, s.Project != "")
	selection.OriginalProjectPresent = original
	if selection.ProjectOrigin == "explicit" {
		selection.ProjectOrigin = s.Sources["project"]
	}
	return selection
}
