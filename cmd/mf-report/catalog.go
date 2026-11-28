package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("catalog", "", newCatalogCommand)
}

func newCatalogCommand() *cobra.Command {
	return &cobra.Command{Use: "catalog", Short: "Export the component catalog"}
}
