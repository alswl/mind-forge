package main

import (
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("init", "", newInitCommand, func(e commandExecution) error {
		target := "."
		if len(e.Args) > 0 {
			target = e.Args[0]
		}
		report, err := content.InitializeRepository(target)
		if err != nil {
			return err
		}
		if e.Application.Quiet {
			return nil
		}
		return output.RenderSuccess(e.Stdout, e.Application.Format, report)
	})
}

func newInitCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "init"}
	return configureCommand(command, item, handler)
}
