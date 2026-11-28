package main

import (
	"fmt"
	"strings"

	"github.com/spf13/cobra"
)

type commandRegistration struct {
	path   string
	parent string
	build  func() *cobra.Command
}

var commandRegistrations []commandRegistration

func registerCommand(path, parent string, build func() *cobra.Command) {
	commandRegistrations = append(commandRegistrations, commandRegistration{path: path, parent: parent, build: build})
}

func addRegisteredCommands(root *cobra.Command) {
	commands := map[string]*cobra.Command{"": root}
	for _, registration := range commandRegistrations {
		command := registration.build()
		if registration.path != strings.TrimSpace(registration.parent+" "+command.Name()) || commands[registration.path] != nil {
			panic(fmt.Sprintf("invalid or duplicate report command registration %q", registration.path))
		}
		commands[registration.path] = command
	}
	for _, registration := range commandRegistrations {
		parent := commands[registration.parent]
		if parent == nil {
			panic(fmt.Sprintf("missing parent %q for report command %q", registration.parent, registration.path))
		}
		parent.AddCommand(commands[registration.path])
	}
}
