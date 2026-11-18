package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term new", "term", newTermNewCommand, func(e commandExecution) error {
		return executeTermNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeTermNew(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	if len(args) == 0 {
		return output.Usage("term name required", "pass the canonical term as the positional argument: `mf term new <NAME>`")
	}
	aliases, tags := termArray(c, "alias"), termArray(c, "tag")
	if boolFlag(c, "dry-run") {
		actions := []string{"create canonical term"}
		lines := []string{"[dry-run] would create term: " + args[0]}
		if len(aliases) > 0 {
			actions = append(actions, "attach alias")
		}
		for _, alias := range aliases {
			actions = append(actions, "  alias: "+alias)
			lines = append(lines, "[dry-run] would attach alias: "+alias+" → "+args[0])
		}
		result := output.VerbResult{Verb: output.Create, Kind: "term", Identity: args[0], DryRun: true, Details: map[string]any{"definition": optionalFlag(c, "definition"), "aliases": aliases, "tags": tags, "planned_actions": actions}}
		return renderTerm(a, stdout, stderr, output.VerbEnvelope(result), strings.Join(lines, "\n"), nil)
	}
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	r, err := scope.New(args[0], term.Input{Definition: optionalFlag(c, "definition"), Description: optionalFlag(c, "description"), Confidence: termFloat(c, "confidence"), Aliases: aliases, Tags: tags}, termArray(c, "misrecognition"), false)
	if err != nil {
		return err
	}
	data := map[string]any{"term": r.Term.Term, "created": r.Created, "added_aliases": r.AddedAliases, "added_tags": r.AddedTags, "added_misrecognitions": r.AddedMisrecognitions}
	text := fmt.Sprintf("term \"%s\" already up to date", r.Term.Term)
	if r.Created {
		text = fmt.Sprintf("created term \"%s\"", r.Term.Term)
		if len(r.AddedAliases) > 0 {
			text += " with alias " + strings.Join(r.AddedAliases, ", ")
		}
	} else {
		if len(r.AddedAliases) > 0 {
			text = fmt.Sprintf("added alias %s to existing term \"%s\"", strings.Join(r.AddedAliases, ", "), r.Term.Term)
		}
	}
	return renderTerm(a, stdout, stderr, data, text, nil)
}
