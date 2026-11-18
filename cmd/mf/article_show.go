package main

import (
	"io"
	"strings"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article show", "article", newArticleShowCommand, func(e commandExecution) error {
		return executeArticleShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeArticleShow(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	all := false && !flagPresent(command, "project") && err != nil && strings.HasPrefix(err.Error(), "could not detect current project;")
	if err != nil && !all {
		return err
	}

	return showArticle(application, project, args[0], stdout, stderr)
}
