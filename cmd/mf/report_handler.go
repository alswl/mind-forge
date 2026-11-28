package main

import (
	"github.com/alswl/mind-forge/internal/reportcli"
	core "github.com/alswl/mind-forge/pkg/services/report"
)

func reportService(e commandExecution) (core.Service, error) {
	return reportcli.Service(e.Application.RepoRoot, stringFlag(e.Command, "worker"), e.Stderr)
}

func reportScope(e commandExecution, service core.Service) (core.Scope, error) {
	return core.ProjectScope(service.Registry, e.Application.CWD, optionalFlag(e.Command, "project"))
}

func reportResult(e commandExecution, value any, err error) error {
	return reportcli.Result(e.Stdout, "mf", e.Application.Format, e.Application.Quiet, value, err)
}

func reportCatalog(e commandExecution, kind core.Kind) error {
	service, err := reportService(e)
	if err != nil {
		return err
	}
	value, err := service.Catalog(kind, stringFlag(e.Command, "id"))
	return reportResult(e, value, err)
}

func reportHelp(e commandExecution) error { return e.Command.Help() }
