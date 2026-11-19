package source

import (
	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/services/content"
)

type ShowResult struct {
	Items    []model.Source
	Selected *model.Source
}

func ShowRequest(project, selector string, filter, kind *string) (ShowResult, error) {
	if kind != nil && *kind == "auto" {
		return ShowResult{}, output.Usage("--type auto is not valid for listing; specify a concrete type", "use --type pdf, file, rss, or web")
	}
	items, err := List(project, filter, kind)
	if err != nil {
		return ShowResult{}, err
	}
	if err := content.ValidateEntityPath(project, selector); err != nil {
		return ShowResult{}, err
	}
	name, err := ResolveSelector(items, selector)
	if err != nil {
		return ShowResult{}, err
	}
	result := ShowResult{Items: items}
	for i := range items {
		if items[i].Name == name {
			result.Selected = &items[i]
			break
		}
	}
	return result, nil
}

func ResolveUpdateRequest(project, selector string) (string, error) {
	if err := content.ValidateEntityPath(project, selector); err != nil {
		return "", err
	}
	items, err := List(project, nil, nil)
	if err != nil {
		return "", err
	}
	return ResolveSelector(items, selector)
}

func InputKind(input string, kind, sourceKind *string) (*string, error) {
	form := ClassifyInput(input)
	if kind == nil && sourceKind == nil {
		return nil, nil
	}
	value := "file"
	if form == URLInput {
		value = "web"
	}
	if kind != nil && *kind != "auto" {
		value = *kind
	}
	if form == URLInput && (value == "pdf" || value == "file") {
		return nil, output.Usage("cannot use --type pdf or --type file with a URL input", "download the file first, then add the local path")
	}
	if form == PathInput && (value == "rss" || value == "web") {
		return nil, output.Usage("cannot use --type rss or --type web with a local file input", "pass an http(s):// URL")
	}
	return &value, nil
}
