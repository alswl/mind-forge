package main

import (
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("report component list", "report component", newReportComponentListCommand, runReportComponentList)
}

func newReportComponentListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "list"}, item, handler)
}

func runReportComponentList(e commandExecution) error { return reportCatalog(e, core.Component) }
