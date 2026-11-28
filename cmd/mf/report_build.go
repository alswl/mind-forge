package main

import (
	"github.com/alswl/mind-forge/internal/reportcli"
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("report build", "report", newReportBuildCommand, runReportBuild)
}

func newReportBuildCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	return configureCommand(&cobra.Command{Use: "build"}, item, handler)
}

func runReportBuild(e commandExecution) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	scope, err := reportScope(e, service)
	if err != nil {
		return reportResult(e, nil, err)
	}
	request := core.Request{
		Slug: e.Args[0], Renderer: stringFlag(e.Command, "renderer"),
		View: stringFlag(e.Command, "view"), Theme: stringFlag(e.Command, "theme"),
	}
	result, err := reportcli.Build(e.Command, service, scope, request, boolFlag(e.Command, "dry-run"))
	return reportResult(e, result, err)
}
