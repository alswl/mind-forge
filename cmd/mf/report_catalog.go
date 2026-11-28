package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report catalog", "report", newReportCatalogCommand, reportHelp)
}

func newReportCatalogCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "catalog"}, item, handler)
}
