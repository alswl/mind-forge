package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report init", "report", newReportInitCommand, runReportInit)
}

func newReportInitCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "init"}, item, handler)
}

func runReportInit(e commandExecution) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	scope, err := reportScope(e, service)
	if err != nil {
		return reportResult(e, nil, err)
	}
	result, err := service.Scaffold(e.Command.Context(), scope, e.Args[0], stringFlag(e.Command, "renderer"))
	return reportResult(e, result, err)
}
