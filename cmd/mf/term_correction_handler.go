package main

import (
	"fmt"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/spf13/cobra"
)

func correctionOption(c *cobra.Command, name string, allowed ...string) (*string, error) {
	v := optionalFlag(c, name)
	if v == nil {
		return nil, nil
	}
	lower := strings.ToLower(*v)
	for _, known := range allowed {
		if lower == known {
			return &lower, nil
		}
	}
	label := name
	if name == "match" || name == "fix" {
		label += " kind"
	}
	return nil, output.Usage(fmt.Sprintf("invalid %s '%s'; expected %s", label, lower, strings.Join(allowed, ", ")), "")
}

func correctionText(c model.Correction) (match, fix, boundary string) {
	match, fix, boundary = c.Match, c.Fix, c.Boundary
	if match == "" {
		match = "word"
	}
	if fix == "" {
		fix = "required"
	}
	if boundary == "" {
		boundary = "standalone"
	}
	return
}
