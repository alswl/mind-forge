package main

import (
	"github.com/alswl/mind-forge/pkg/common/model"
)

func sourceLocation(item model.Source) string {
	if item.Path != nil {
		return *item.Path
	}
	if item.URL != nil {
		return *item.URL
	}
	return "-"
}
