package main

import (
	"fmt"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/version"
	"github.com/alswl/mind-forge/pkg/services/versioncheck"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("version", "", newVersionCommand, func(e commandExecution) error {
		if boolFlag(e.Command, "check") {
			return runVersionCheck(e)
		}
		if e.Application.Quiet {
			return nil
		}
		metadata := version.Current()
		if e.Application.Format == "json" {
			return output.RenderSuccess(e.Stdout, e.Application.Format, metadata)
		}
		return output.RenderRaw(e.Stdout, e.Application.Format, metadata.Text())
	})
}

func newVersionCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "version"}
	return configureCommand(command, item, handler)
}

func runVersionCheck(e commandExecution) error {
	result, err := versioncheck.Check(e.Command.Context(), version.Current().Version)
	if err != nil {
		hint := "Try again or consult the installation guide at " + versioncheck.InstallURL
		return &output.Error{Kind: "version_check_failed", Message: "version check failed: " + err.Error(), Hint: &hint, Cause: err}
	}
	if e.Application.Quiet {
		return nil
	}
	if e.Application.Format == "json" {
		return output.RenderSuccess(e.Stdout, e.Application.Format, result)
	}
	var status string
	switch result.Status {
	case versioncheck.UpdateAvailable:
		status = "Update available. Install instructions: " + result.UpgradeURL
	case versioncheck.UpToDate:
		status = "Up to date."
	case versioncheck.Ahead:
		status = "Current version is ahead of the latest stable release."
	}
	message := fmt.Sprintf("Current version: %s\nLatest stable version: %s\n%s\n", result.CurrentVersion, result.LatestVersion, status)
	return output.RenderRaw(e.Stdout, e.Application.Format, message)
}
