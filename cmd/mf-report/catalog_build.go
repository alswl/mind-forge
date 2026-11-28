package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("catalog build", "catalog", newCatalogBuildCommand)
}

func newCatalogBuildCommand() *cobra.Command {
	command := &cobra.Command{
		Use: "build", Short: "Export the offline component catalog", Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			service, err := serviceFor(command)
			if err != nil {
				return err
			}
			scope, err := scopeFor(command)
			if err != nil {
				return err
			}
			result, err := service.BuildCatalog(command.Context(), scope, "", stringFlag(command, "theme"))
			return reportResult(command, result, err)
		},
	}
	command.Flags().String("theme", "system", "Report theme (system, light or dark)")
	command.Flags().String("worker", "", "Override the executable for the report worker")
	return command
}
