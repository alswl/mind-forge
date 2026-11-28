package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report fragment build", "report fragment", newReportFragmentBuildCommand, runReportFragmentBuild)
}

func newReportFragmentBuildCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "build"}, item, handler)
}

func runReportFragmentBuild(e commandExecution) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	scope, err := reportScope(e, service)
	if err != nil {
		return reportResult(e, nil, err)
	}
	result, err := service.BuildFragment(e.Command.Context(), scope, e.Args[0], e.Args[1])
	return reportResult(e, result, err)
}
