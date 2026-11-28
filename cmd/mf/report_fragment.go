package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report fragment", "report", newReportFragmentCommand, reportHelp)
}

func newReportFragmentCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "fragment"}, item, handler)
}
