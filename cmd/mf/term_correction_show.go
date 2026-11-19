package main

import (
	"fmt"
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction show", "term correction", newTermCorrectionShowCommand, func(e commandExecution) error {
		return executeTermCorrectionShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermCorrectionShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeTermCorrectionShow(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]

	corr, err := scope.QueryCorrection(name, args[1])
	if err != nil {
		return err
	}
	match, fix, boundary := correctionText(corr)
	text := fmt.Sprintf("correction \"%s\" → \"%s\"\n  match: %s\n  fix: %s\n  boundary: %s", corr.Original, corr.Correct, match, fix, boundary)
	if corr.Pinyin != nil {
		text += "\n  pinyin: " + *corr.Pinyin
	}
	return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "correction": corr}, text, nil)
}
