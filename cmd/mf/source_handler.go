package main

import (
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func sourceInputKind(command *cobra.Command, input string) (*string, error) {
	form := source.ClassifyInput(input)
	kind := optionalFlag(command, "file-kind")
	if kind == nil && optionalFlag(command, "source-kind") == nil {
		return nil, nil
	}
	value := "file"
	if form == source.URLInput {
		value = "web"
	}
	if kind != nil && *kind != "auto" {
		value = *kind
	}
	if form == source.URLInput && (value == "pdf" || value == "file") {
		return nil, output.Usage("cannot use --type pdf or --type file with a URL input", "download the file first, then add the local path")
	}
	if form == source.PathInput && (value == "rss" || value == "web") {
		return nil, output.Usage("cannot use --type rss or --type web with a local file input", "pass an http(s):// URL")
	}
	return &value, nil
}

func sourceLocation(item model.Source) string {
	if item.Path != nil {
		return *item.Path
	}
	if item.URL != nil {
		return *item.URL
	}
	return "-"
}
