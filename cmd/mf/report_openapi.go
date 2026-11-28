package main

import (
	"github.com/alswl/mind-forge/internal/reportcli"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("report openapi", "report", newReportOpenAPICommand, runReportOpenAPI)
}

func newReportOpenAPICommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "openapi"}, item, handler)
}

func runReportOpenAPI(e commandExecution) error { return reportcli.OpenAPI(e.Stdout) }
