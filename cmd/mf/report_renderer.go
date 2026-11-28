package main

import "github.com/spf13/cobra"

func init() {
	registerCommand("report renderer", "report", newReportRendererCommand, reportHelp)
}

func newReportRendererCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "renderer"}, item, handler)
}
