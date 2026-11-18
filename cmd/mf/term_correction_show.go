package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
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

	return executeTermCorrectionShowDetails(c, a, args, stdout, stderr)
}

func executeTermCorrectionShowDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]

	{

		terms, err := scope.Load(true)
		if err != nil {
			return err
		}
		var tm *model.Term
		for i := range terms {
			if terms[i].Term == name {
				tm = &terms[i]
				break
			}
		}
		if tm == nil {
			return &output.Error{Kind: "not_found", Message: "term '" + name + "' not found"}
		}

		for _, corr := range tm.Corrections {
			if corr.Original == args[1] {
				match, fix, boundary := correctionText(corr)
				text := fmt.Sprintf("correction \"%s\" → \"%s\"\n  match: %s\n  fix: %s\n  boundary: %s", corr.Original, corr.Correct, match, fix, boundary)
				if corr.Pinyin != nil {
					text += "\n  pinyin: " + *corr.Pinyin
				}
				return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "correction": corr}, text, nil)
			}
		}
		return &output.Error{Kind: "not_found", Message: fmt.Sprintf("correction \"%s\" not found on term \"%s\"", args[1], name)}
	}
}
