package main

import (
	server "github.com/alswl/mind-forge/pkg/server/report"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("report serve", "report", newReportServeCommand, runReportServe)
}

func newReportServeCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "serve"}, item, handler)
}

func runReportServe(e commandExecution) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	scope, err := reportScope(e, service)
	if err != nil {
		return reportResult(e, nil, err)
	}
	return server.Serve(e.Command.Context(), stringFlag(e.Command, "listen"), service, scope, e.Stderr)
}
