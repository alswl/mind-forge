package content

import (
	"fmt"
	"regexp"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/outcome"
)

type finalLintRule struct {
	match      *regexp.Regexp
	suggestion string
}

var defaultFinalLintRules = []finalLintRule{
	{regexp.MustCompile("\\bW[0-9]+\\b"), "replace the internal week identifier with a date or public milestone"},
	{regexp.MustCompile("(?i)\\balign[-_][0-9]+\\b"), "replace the internal project ID with a public name"},
	{regexp.MustCompile("\\b(?:refs/heads|feature|feat|fix|bugfix|hotfix|refactor|release)/[[:alnum:]_.-]+(?:/[[:alnum:]_.-]+)*"), "refer to the public change or release instead of the Git branch"},
	{regexp.MustCompile("不跨月|占位|示意|待排期|批量导入|[（【\\[(]规划[）】\\])]"), "replace the production note with finished copy or remove it"},
	{regexp.MustCompile("出自|来源为"), "cite the source directly and remove the production attribution wording"},
}

func compileFinalLintRules(config *model.ArticleLintConfig) ([]finalLintRule, error) {
	if config == nil {
		return defaultFinalLintRules, nil
	}
	var rules []finalLintRule
	if !config.Final.DisableDefaults {
		rules = append(rules, defaultFinalLintRules...)
	}
	for index, rule := range config.Final.Rules {
		field := fmt.Sprintf("article_lint.final.rules[%d]", index+1)
		if (rule.Literal == nil) == (rule.Pattern == nil) {
			return nil, outcome.Usage(field+" requires exactly one of literal or pattern", "")
		}
		suggestion := strings.TrimSpace(rule.Suggestion)
		if suggestion == "" {
			return nil, outcome.Usage(field+" requires a nonempty suggestion", "")
		}
		var pattern string
		if rule.Literal != nil {
			pattern = regexp.QuoteMeta(*rule.Literal)
		} else {
			pattern = *rule.Pattern
		}
		match, err := regexp.Compile(pattern)
		if err != nil {
			return nil, outcome.Usage(fmt.Sprintf("%s has an invalid pattern: %s", field, err), "")
		}
		if strings.TrimSpace(pattern) == "" || strings.ContainsAny(pattern, "\r\n") || match.MatchString("") {
			return nil, outcome.Usage(field+" requires a nonempty, single-line match", "")
		}
		rules = append(rules, finalLintRule{match: match, suggestion: suggestion})
	}
	return rules, nil
}

func appendFinalLintIssues(issues []model.LintIssue, path, source string, rules []finalLintRule) []model.LintIssue {
	visibility, err := markdown.BlockVisibility(source)
	if err != nil {
		return append(issues, model.LintIssue{
			Severity: "error", Kind: "mind_forge_visibility_invalid", Message: err.Error(), Path: path,
		})
	}
	if visibility == "private" {
		return issues
	}
	_, body, _, _ := markdown.InitialFrontMatter(source)
	offset := strings.Count(source[:len(source)-len(body)], "\n")
	lines := strings.Split(body, "\n")
	private := markdown.PrivateCallouts(lines)
	for i, line := range lines {
		number := i + 1
		if len(private) > 0 && number >= private[0].StartLine {
			if number == private[0].EndLine {
				private = private[1:]
			}
			continue
		}
		for _, rule := range rules {
			for _, match := range rule.match.FindAllString(strings.TrimSuffix(line, "\r"), -1) {
				if match == "" {
					continue
				}
				issues = append(issues, model.LintIssue{
					Severity: "warning", Kind: "final_leak", Path: path, Line: offset + number,
					Message: fmt.Sprintf("%q: %s", match, rule.suggestion),
				})
			}
		}
	}
	return issues
}
