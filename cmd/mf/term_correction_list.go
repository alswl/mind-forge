package main

import (
	"fmt"
	"io"
	"strings"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction list", "term correction", newTermCorrectionListCommand, func(e commandExecution) error {
		return executeTermCorrectionList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermCorrectionListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeTermCorrectionList(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]

	tm, err := scope.QueryCorrections(name)
	if err != nil {
		return err
	}

	lines := []string{}
	if len(tm.Corrections) == 0 {
		lines = append(lines, fmt.Sprintf("no corrections for term \"%s\"", name))
	} else {
		lines = append(lines, fmt.Sprintf("corrections for term \"%s\":", name))
		for _, corr := range tm.Corrections {
			match, fix, _ := correctionText(corr)
			lines = append(lines, fmt.Sprintf("  \"%s\" → \"%s\" [match=%s, fix=%s]", corr.Original, corr.Correct, match, fix))
		}
	}
	return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "corrections": tm.Corrections}, strings.Join(lines, "\n"), nil)

}
