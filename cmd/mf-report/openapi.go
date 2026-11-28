package main

import (
	"github.com/alswl/mind-forge/internal/reportcli"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("openapi", "", newOpenAPICommand)
}

func newOpenAPICommand() *cobra.Command {
	return &cobra.Command{
		Use: "openapi", Short: "Print the OpenAPI 3.1 YAML contract", Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error { return reportcli.OpenAPI(command.OutOrStdout()) },
	}
}
