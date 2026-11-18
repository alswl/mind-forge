package content

import (
	"github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
)

func LintProjectRequest(root, cwd string, selector *string, rules []string, fix bool, severity *string) ([]map[string]any, ProjectLintSummary, error) {

	rules, err := NormalizeProjectLintRules(rules)
	if err != nil {
		return nil, ProjectLintSummary{}, err
	}

	objects := []map[string]any{}
	summary := ProjectLintSummary{}
	if selector := selector; selector != nil {
		project, err := app.ResolveProject(root, selector, cwd, false)
		if err != nil {
			return nil, ProjectLintSummary{}, err
		}
		issues, counts, err := LintProject(project, rules, fix)
		if err != nil {
			return nil, ProjectLintSummary{}, err
		}
		summary = counts
		for _, issue := range issues {
			object, err := outcome.Object(issue)
			if err != nil {
				return nil, ProjectLintSummary{}, err
			}
			objects = append(objects, object)
		}
	} else {
		objects, summary, err = LintRepository(root, rules, fix)
		if err != nil {
			return nil, ProjectLintSummary{}, err
		}
	}
	if severity != nil {
		rank := lintSeverityRank(*severity)
		filtered := []map[string]any{}
		summary.Errors, summary.Warnings = 0, 0
		for _, issue := range objects {
			severity, _ := issue["severity"].(string)
			if lintSeverityRank(severity) > rank {
				continue
			}
			filtered = append(filtered, issue)
			// The frozen explicit severity filter counts all visible issues, including fixed ones.
			if severity == "error" {
				summary.Errors++
			} else if severity == "warning" {
				summary.Warnings++
			}
		}
		objects = filtered
	}
	return objects, summary, nil
}
func lintSeverityRank(value string) int {
	switch value {
	case "error":
		return 0
	case "warning":
		return 1
	default:
		return 2
	}
}
