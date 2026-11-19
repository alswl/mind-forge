package content

import (
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
)

type MindProjectInspection struct {
	Identity    string
	Layout      string
	Minds       []model.Mind
	OutputCount int
}

type ProjectInspection struct {
	Legacy *model.ProjectDetails
	Mind   *MindProjectInspection
}

func InspectProject(root, cwd, selector string) (ProjectInspection, error) {
	project, err := app.ResolveProject(root, &selector, cwd, false)
	if err != nil {
		return ProjectInspection{}, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return ProjectInspection{}, err
	}
	if config.ContentLayout != nil {
		minds, err := Discover(root, project)
		if err != nil {
			return ProjectInspection{}, err
		}
		identity, err := ProjectIdentityPath(root, project)
		if err != nil {
			return ProjectInspection{}, err
		}
		count := 0
		for _, mind := range minds {
			count += len(mind.Outputs)
		}
		return ProjectInspection{Mind: &MindProjectInspection{Identity: identity, Layout: *config.ContentLayout, Minds: minds, OutputCount: count}}, nil
	}
	details, err := ProjectDetails(project, selector)
	if err != nil {
		return ProjectInspection{}, err
	}
	return ProjectInspection{Legacy: &details}, nil
}
