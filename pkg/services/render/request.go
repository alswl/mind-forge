package render

import (
	"path/filepath"

	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
)

func GenerateRequest(root, cwd string, projectSelector, article *string, template, htmlForm string) (model.GeneratedPrompt, error) {
	templates, err := Templates(root)
	if err != nil {
		return model.GeneratedPrompt{}, err
	}

	project, err := app.ResolveProject(root, projectSelector, cwd, false)
	if err != nil {
		return model.GeneratedPrompt{}, err
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return model.GeneratedPrompt{}, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return model.GeneratedPrompt{}, err
	}
	generated, err := Generate(project, filepath.Base(project), layout.BuildOutput, config.Build.Format, article, template, htmlForm, templates)
	if err != nil {
		return model.GeneratedPrompt{}, err
	}
	return generated, nil
}
