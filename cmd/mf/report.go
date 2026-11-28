package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report", "", newReportCommand, reportHelp)
}

func newReportCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "report"}, item, handler)
}
