package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("build", "", newBuildCommand, func(e commandExecution) error {
		return buildCommand(e.Command, e.Application, e.Args, e.Stdout, e.Stderr, e.ProcessIO)
	})
}

func newBuildCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "build"}
	return configureCommand(command, item, handler)
}
