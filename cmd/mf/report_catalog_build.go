package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report catalog build", "report catalog", newReportCatalogBuildCommand, runReportCatalogBuild)
}

func newReportCatalogBuildCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "build"}, item, handler)
}

func runReportCatalogBuild(e commandExecution) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	scope, err := reportScope(e, service)
	if err != nil {
		return reportResult(e, nil, err)
	}
	result, err := service.BuildCatalog(e.Command.Context(), scope, e.Args[0], stringFlag(e.Command, "theme"))
	return reportResult(e, result, err)
}
