package main

import (
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("report renderer list", "report renderer", newReportRendererListCommand, runReportRendererList)
}

func newReportRendererListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "list"}, item, handler)
}

func runReportRendererList(e commandExecution) error { return reportCatalog(e, core.Renderer) }
