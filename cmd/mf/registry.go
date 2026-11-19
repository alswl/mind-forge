package main

import (
	"fmt"
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/build"
	"github.com/spf13/cobra"
)

type commandExecution struct {
	ProcessIO      build.ProcessIO
	Selection      ResolvedSelection
	Command        *cobra.Command
	Application    *app.Context
	Args           []string
	Stdout, Stderr io.Writer
}

type commandFactory func(CommandSpec, Handler) (*cobra.Command, error)
type commandAction func(commandExecution) error

type commandRegistration struct {
	parent string
	build  commandFactory
	run    commandAction
}

var commandRegistrations = map[string]commandRegistration{}

// Register factories, rather than mutable commands, to isolate flag state for
// repeated executions in the same process.
func registerCommand(path, parent string, build commandFactory, run commandAction) {
	if _, exists := commandRegistrations[path]; exists {
		panic(fmt.Sprintf("duplicate command registration %q", path))
	}
	commandRegistrations[path] = commandRegistration{parent: parent, build: build, run: run}
}
