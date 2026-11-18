package main

import (
	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
)

func checkArticleLayout(application *app.Context, selector *string) error {
	project, err := app.ResolveProject(application.RepoRoot, selector, application.CWD, false)
	if err != nil {
		// Keep command-specific context errors and repository-wide listing.
		return nil
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return err
	}
	if config.ContentLayout != nil {
		return output.Usage("this project uses minds and outputs; use `mf mind` or `mf output`; Article lifecycle commands do not operate on this layout", "")
	}
	return nil
}
