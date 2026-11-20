package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/publish"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish update", "publish", newPublishUpdateCommand, func(e commandExecution) error {
		return executePublishUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newPublishUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executePublishUpdate(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	selector := args[0]
	project, err := content.ProjectFor(a.RepoRoot, a.CWD, optionalFlag(c, "project"), &selector)
	if err != nil {
		return err
	}
	{

		status, url := optionalFlag(c, "status"), optionalFlag(c, "target-url")
		warnings := []string{}
		if status != nil {
			if err := output.WarnDeprecation(stderr, "--status", "--set status=<value>", !a.Color); err != nil {
				return err
			}
		}
		if url != nil {
			if err := output.WarnDeprecation(stderr, "--target-url", "--set url=<value>", !a.Color); err != nil {
				return err
			}
		}
		sets, _ := c.Flags().GetStringArray("set")
		for _, pair := range sets {
			key, value, ok := strings.Cut(pair, "=")
			if !ok {
				continue
			}
			switch key {
			case "status":
				if value == "draft" || value == "published" || value == "archived" {
					copy := value
					status = &copy
				}
			case "url":
				copy := value
				url = &copy
			}
		}
		result, location, err := publish.Update(a.RepoRoot, project, selector, publish.UpdateOptions{Target: stringFlag(c, "target"), Status: status, URL: url, FromPath: optionalFlag(c, "from-path"), DryRun: boolFlag(c, "dry-run")})
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
		urlText, dateText := "null", "null"
		if result.Record.TargetURL != nil {
			urlText = *result.Record.TargetURL
		}
		if result.Record.PublishedAt != nil {
			dateText = *result.Record.PublishedAt
		}
		text := fmt.Sprintf("article      %s\ntarget       %s\naction       %s\nstatus       %s\ntarget_url   %s\npublished_at %s\ndry_run      %t", result.Article, result.TargetName, result.Action, result.Record.Status, urlText, dateText, result.DryRun)
		if result.Action == "rebound" {
			text += "\npath         " + result.Record.Path
		}
		return renderTerm(a, stdout, stderr, data, text, warnings)
	}
}
