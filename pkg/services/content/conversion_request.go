package content

import (
	"context"
	"path/filepath"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func ArticleConversionPaths(project string, selector *string) ([]string, error) {
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return nil, err
	}
	paths := []string{}
	if index.Value.Articles != nil {
		for _, article := range *index.Value.Articles {
			paths = append(paths, article.Path)
		}
	}
	if selector := selector; selector != nil {
		path, err := ResolveArticleSelector(project, *selector)
		if err != nil {
			return nil, err
		}
		paths = []string{path}
	}
	return paths, nil
}

func InferArticleConversion(root, project string, paths []string) (string, map[string]int, error) {
	choices := []string{}
	counts := map[string]int{}
	for _, candidate := range []string{"to_single_file", "to_directory"} {
		items, err := InspectArticleConversions(root, project, paths, candidate, false)
		if err != nil {
			return "", nil, err
		}
		for _, item := range items {
			if item.Eligible {
				counts[candidate]++
			}
		}
		if counts[candidate] > 0 {
			choices = append(choices, candidate)
		}
	}
	if len(choices) == 0 {
		return "", nil, output.Usage("no eligible articles found for conversion", "verify that the project has articles that can be converted")
	}
	if len(choices) > 1 {
		return "", nil, output.Usage("ambiguous conversion direction; both --to-single-file and --to-directory are possible", "pass --to-single-file or --to-directory to specify the desired direction")
	}
	return choices[0], counts, nil
}

func ConvertArticleRequest(ctx context.Context, root, project string, paths []string, direction, source string, merge, dryRun bool) (model.ConversionSummary, error) {
	inspections, err := InspectArticleConversions(root, project, paths, direction, merge)
	if err != nil {
		return model.ConversionSummary{}, err
	}

	summary := model.ConversionSummary{Kind: "article", Direction: direction, DirectionSource: source, DryRun: dryRun, ScannedCount: uint64(len(inspections)), Converted: model.JSONArray[model.ConversionResult]{}, Skipped: model.JSONArray[model.ConversionResult]{}, Failed: model.JSONArray[model.ConversionResult]{}}
	for _, inspection := range inspections {
		if err := ctx.Err(); err != nil {
			return summary, err
		}
		if !inspection.Eligible {
			summary.Skipped = append(summary.Skipped, inspection.Result("skipped", false, false))
			continue
		}
		if dryRun {
			summary.Converted = append(summary.Converted, inspection.Result("would_convert", false, false))
			continue
		}
		mutation, err := PlanArticleConversion(root, project, inspection, time.Time{})
		if err == nil {
			err = ApplyMutation(ctx, root, mutation)
		}
		if err != nil {
			result := inspection.Result("failed", false, false)
			reason := err.Error()
			result.Reason = &reason
			summary.Failed = append(summary.Failed, result)
		} else {
			summary.Converted = append(summary.Converted, inspection.Result("converted", true, true))
		}
	}
	summary.ConvertedCount, summary.SkippedCount, summary.FailedCount = uint64(len(summary.Converted)), uint64(len(summary.Skipped)), uint64(len(summary.Failed))
	return summary, nil
}
