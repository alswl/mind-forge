package main

import (
	"encoding/json"
	"fmt"
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/publish"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish run", "publish", newPublishRunCommand, func(e commandExecution) error {
		return executePublishRun(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newPublishRunCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "run"}
	return configureCommand(command, item, handler)
}

func executePublishRun(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	selector := args[0]
	project, err := content.ProjectFor(a.RepoRoot, a.CWD, optionalFlag(c, "project"), &selector)
	if err != nil {
		return err
	}

	result, location, warnings, err := publish.Run(a.RepoRoot, project, selector, publish.RunOptions{Context: c.Context(), Target: optionalFlag(c, "target"), DryRun: boolFlag(c, "dry-run"), Force: boolFlag(c, "force"), StripFrontMatter: boolFlag(c, "strip-front-matter"), KeepFrontMatter: boolFlag(c, "keep-front-matter")})
	if err != nil {
		return err
	}
	data, err := jsonObject(result)
	if err != nil {
		return err
	}
	if location != nil {
		data["location"] = location
	}
	text := ""
	if result.Local != nil {
		o := result.Local
		text = fmt.Sprintf("target      %s\ntype        local\narticle     %s\nsource      %s\ndestination %s\nsize        %d bytes\ndry_run     %t", o.TargetName, o.Article, o.Source, o.Destination, o.SizeBytes, o.DryRun)
	} else {
		o := result.YuquePrompt
		envelope, err := jsonObject(o)
		if err != nil {
			return err
		}
		delete(envelope, "prompt")
		raw, err := json.Marshal(envelope)
		if err != nil {
			return err
		}
		text = "### Publish Prompt\n\n" + o.Prompt + "\n\n### Envelope\n\n```json\n" + string(raw) + "\n```"
	}
	return renderTerm(a, stdout, stderr, data, text, warnings)
}
