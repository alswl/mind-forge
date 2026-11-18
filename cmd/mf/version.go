package main

import (
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/version"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("version", "", newVersionCommand, func(e commandExecution) error {
		if e.Application.Quiet {
			return nil
		}
		metadata := version.Current()
		if e.Application.Format == "json" {
			return output.RenderSuccess(e.Stdout, e.Application.Format, metadata)
		}
		return output.RenderRaw(e.Stdout, e.Application.Format, metadata.Text())
	})
}

func newVersionCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "version"}
	return configureCommand(command, item, handler)
}
