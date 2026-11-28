package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report component", "report", newReportComponentCommand, reportHelp)
}

func newReportComponentCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "component"}, item, handler)
}
