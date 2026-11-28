package main

import (
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func newListCommand() *cobra.Command {
	command := &cobra.Command{
		Use: "list renderers|components", Short: "List registered renderers or components",
		Args:      cobra.MatchAll(cobra.ExactArgs(1), cobra.OnlyValidArgs),
		ValidArgs: []string{"renderers", "components"},
		RunE: func(command *cobra.Command, args []string) error {
			service, err := serviceFor(command)
			if err != nil {
				return err
			}
			kind := core.Renderer
			if args[0] == "components" {
				kind = core.Component
			}
			result, err := service.Catalog(kind, stringFlag(command, "id"))
			return reportResult(command, result, err)
		},
	}
	command.Flags().String("id", "", "Look up one exact registered ID")
	return command
}
