package main

import (
	"github.com/alswl/mind-forge/internal/reportcli"
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func newBuildCommand() *cobra.Command {
	var request core.Request
	var dryRun bool
	command := &cobra.Command{
		Use: "build", Short: "Export a report from the working directory", Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			service, err := serviceFor(command)
			if err != nil {
				return err
			}
			scope, err := scopeFor(command)
			if err != nil {
				return err
			}
			result, err := reportcli.Build(command, service, scope, request, dryRun)
			return reportResult(command, result, err)
		},
	}
	flags := command.Flags()
	flags.StringVar(&request.Entry, "entry", "", "Report TSX entry (relative to workdir)")
	flags.StringVar(&request.Data, "data", "", "Report JSON data (relative to workdir)")
	flags.StringVar(&request.Renderer, "renderer", "", "Override the registered renderer")
	flags.StringVar(&request.View, "view", "chart", "Report view (chart or table)")
	flags.StringVar(&request.Theme, "theme", "system", "Report theme (system, light or dark)")
	flags.String("worker", "", "Override the executable for the report worker")
	flags.BoolVarP(&dryRun, "dry-run", "n", false, "Show the build plan without running a worker")
	command.MarkFlagsRequiredTogether("entry", "data")
	return command
}
