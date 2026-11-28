package main

import (
	server "github.com/alswl/mind-forge/pkg/server/report"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("serve", "", newServeCommand)
}

func newServeCommand() *cobra.Command {
	command := &cobra.Command{
		Use: "serve", Short: "Serve a report from the working directory", Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			service, err := serviceFor(command)
			if err != nil {
				return err
			}
			scope, err := scopeFor(command)
			if err != nil {
				return err
			}
			return server.Serve(command.Context(), stringFlag(command, "listen"), service, scope, command.ErrOrStderr())
		},
	}
	command.Flags().String("listen", server.DefaultAddress, "Loopback listen address")
	command.Flags().String("worker", "", "Override the executable for the report worker")
	return command
}
