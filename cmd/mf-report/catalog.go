package main

import "github.com/spf13/cobra"

func newCatalogCommand() *cobra.Command {
	command := &cobra.Command{Use: "catalog", Short: "Export the component catalog"}
	command.AddCommand(newCatalogBuildCommand())
	return command
}
