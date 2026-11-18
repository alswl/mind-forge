package main

import (
	"fmt"
	"io"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
)

func termArray(c *cobra.Command, name string) []string {
	v, _ := c.Flags().GetStringArray(name)
	if v == nil {
		return []string{}
	}
	return v
}

func termFloat(c *cobra.Command, name string) *float64 {
	if !flagPresent(c, name) {
		return nil
	}
	v, _ := c.Flags().GetFloat64(name)
	return &v
}

func termScope(c *cobra.Command, a *app.Context) (term.Scope, error) {
	if p := optionalFlag(c, "project"); p != nil {
		root, err := app.ResolveProject(a.RepoRoot, p, a.CWD, false)
		return term.Scope{Root: root, Kind: "project"}, err
	}
	return term.Scope{Root: a.RepoRoot, Kind: "global"}, nil
}

func termMissing(err error) bool {
	return term.IsMissingTerm(err)
}

func termFallback[T any](c *cobra.Command, a *app.Context, name string, operation func(term.Scope) (T, error)) (T, string, []string, error) {
	scope, err := termScope(c, a)
	if err != nil {
		var zero T
		return zero, "", nil, err
	}
	value, err := operation(scope)
	if err != nil && scope.Kind == "project" && termMissing(err) {
		value, err = operation(term.Scope{Root: a.RepoRoot, Kind: "global"})
		if err == nil {
			return value, "global", []string{fmt.Sprintf("-p %s was ignored; the write applied to global scope because \"%s\" does not exist as a project-local term.", stringFlag(c, "project"), name)}, nil
		}
	}
	return value, scope.Kind, nil, err
}

func renderTerm(a *app.Context, stdout, stderr io.Writer, data any, text string, warnings []string) error {
	return renderTermWarningPrefix(a, stdout, stderr, data, text, warnings, "WARN: ")
}

func renderCorrection(a *app.Context, stdout, stderr io.Writer, data any, text string, warnings []string) error {
	return renderTermWarningPrefix(a, stdout, stderr, data, text, warnings, "warning: ")
}

func renderTermWarningPrefix(a *app.Context, stdout, stderr io.Writer, data any, text string, warnings []string, prefix string) error {
	for _, w := range warnings {
		if _, err := fmt.Fprintln(stderr, prefix+w); err != nil {
			return err
		}
	}
	if a.Quiet {
		return nil
	}
	if a.Format == "json" {
		if len(warnings) > 0 {
			payload, err := jsonObject(data)
			if err != nil {
				return err
			}
			if payload == nil {
				return fmt.Errorf("success data must be an object")
			}
			target := payload
			if _, status := payload["status"]; status {
				if _, command := payload["command"]; command {
					if nested, exists := payload["data"]; exists {
						var ok bool
						target, ok = nested.(map[string]any)
						if !ok || target == nil {
							return fmt.Errorf("success data must be an object")
						}
					}
				}
			}
			target["warnings"] = warnings
			return output.RenderSuccess(stdout, "json", payload)
		}
		return output.RenderSuccess(stdout, "json", data)
	}
	return output.RenderRaw(stdout, "text", text)
}

func termVerb(a *app.Context, stdout, stderr io.Writer, result output.VerbResult, warnings []string) error {
	profile := commandProfile(stdout, a)
	return renderTermWarningPrefix(a, stdout, stderr, output.VerbEnvelope(result), output.RenderVerb(result, output.DisplayOptions{RepoRoot: a.RepoRoot, Hyperlinks: profile.Hyperlinks}), warnings, "WARN: ")
}

func termListCommand(c *cobra.Command, a *app.Context, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	project := ""
	if scope.Kind == "project" {
		project = scope.Root
	}
	items, err := term.List(a.RepoRoot, project, term.ListFilter{Filter: stringFlag(c, "filter"), Scope: stringFlag(c, "scope"), Tags: termArray(c, "tag"), Aliases: termArray(c, "alias"), HasCorrection: boolFlag(c, "has-correction")})
	if err != nil {
		return err
	}
	view := output.List{Headers: []string{"TERM", "DEFINITION", "ALIASES", "TAGS", "CORRECTIONS"}, Plural: "terms"}
	for _, tm := range items {
		definition, aliases, tags, count := "-", "-", "-", "-"
		if tm.Definition != nil {
			definition = *tm.Definition
		}
		if len(tm.Aliases) > 0 {
			aliases = strings.Join(tm.Aliases, ", ")
		}
		if len(tm.Tags) > 0 {
			tags = strings.Join(tm.Tags, ", ")
		}
		if len(tm.Corrections) > 0 {
			count = strconv.Itoa(len(tm.Corrections))
		}
		view.Rows = append(view.Rows, output.TextCells(tm.Term.Term, definition, aliases, tags, count))
	}
	text := output.RenderList(view, output.ListOptionsFromProfile(commandProfile(stdout, a), boolFlag(c, "no-headers"), boolFlag(c, "no-trunc")))
	return renderTerm(a, stdout, stderr, output.JSONCollection("terms", items), text, nil)
}

func termShowCommand(c *cobra.Command, a *app.Context, name string, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	tm, err := scope.Show(name)
	if err != nil && scope.Kind == "project" && termMissing(err) {
		scope = term.Scope{Root: a.RepoRoot, Kind: "global"}
		tm, err = scope.Show(name)
	}
	if termMissing(err) {
		return output.Usage(err.Error(), "use `mf term list` to see all terms")
	}
	if err != nil {
		return err
	}
	data, err := jsonObject(tm)
	if err != nil {
		return err
	}
	data["kind"], data["identity"] = "term", tm.Term
	fields := []output.ShowField{}
	add := func(label, value string) {
		fields = append(fields, output.ShowField{Label: label, Value: output.ShowValue{Text: value}})
	}
	value := func(p *string) string {
		if p == nil {
			return "-"
		}
		return *p
	}
	joined := func(v []string) string {
		if len(v) == 0 {
			return "-"
		}
		return strings.Join(v, ", ")
	}
	add("Term", tm.Term)
	add("Definition", value(tm.Definition))
	add("Description", value(tm.Description))
	add("Aliases", joined(tm.Aliases))
	add("Tags", joined(tm.Tags))
	confidence := "-"
	if tm.Confidence != nil {
		confidence = fmt.Sprintf("%.2f", *tm.Confidence)
	}
	add("Confidence", confidence)
	count := "-"
	if len(tm.Corrections) > 0 {
		count = strconv.Itoa(len(tm.Corrections))
	}
	add("Corrections", count)
	if optionalFlag(c, "project") != nil {
		data["scope"] = scope.Kind
		add("Scope", scope.Kind)
	}
	block := output.ShowBlock{Kind: "term", Identity: tm.Term, Fields: fields}
	if len(tm.Corrections) > 0 {
		section := output.ShowSection{Heading: "Corrections"}
		for _, corr := range tm.Corrections {
			section.Fields = append(section.Fields, output.ShowField{Label: "Correction", Value: output.ShowValue{Text: fmt.Sprintf("\"%s\" → \"%s\"", corr.Original, corr.Correct)}})
		}
		block.Sections = append(block.Sections, section)
	}
	return renderTerm(a, stdout, stderr, data, output.RenderShow(block, output.DisplayOptions{RepoRoot: a.RepoRoot}), nil)
}
