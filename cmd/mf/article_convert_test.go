package main

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestConversionTextUsesPublishedDirectionFlag(t *testing.T) {
	summary := model.ConversionSummary{Direction: "to_single_file", DryRun: true, ConvertedCount: 1, ScannedCount: 1, Converted: model.JSONArray[model.ConversionResult]{{SourcePath: "docs/post", TargetPath: "docs/post.md"}}}
	got := renderArticleConversion(summary)
	if !strings.Contains(got, "[dry-run] article convert --to-single-file: 1 would convert, 0 skipped, 0 failed") {
		t.Fatal(got)
	}
}
