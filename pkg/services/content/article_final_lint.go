package content

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"go.yaml.in/yaml/v3"
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

func LintFinalArticles(ctx context.Context, project string, status *string) ([]model.LintIssue, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if err := validateArticleStatus(status); err != nil {
		return nil, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	rules, err := compileFinalLintRules(config.ArticleLint)
	if err != nil {
		return nil, err
	}
	index, _, err := discoverArticles(ctx, project, config, time.Time{})
	if err != nil {
		return nil, err
	}
	paths := finalArticlePaths(*index.Articles, config.Build.Articles, status)
	issues := []model.LintIssue{}
	seen := map[string]bool{}
	for _, relative := range paths {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		if filepath.IsAbs(relative) {
			relative, err = filepath.Rel(project, relative)
			if err != nil {
				return nil, err
			}
		}
		full, err := store.CanonicalWithin(project, filepath.Join(project, relative))
		if err != nil {
			return nil, err
		}
		info, err := os.Stat(full)
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			return nil, err
		}
		files := []string{relative}
		if info.IsDir() {
			files, err = finalArticleBlocks(full, relative)
			if err != nil {
				return nil, err
			}
		}
		for _, file := range files {
			if seen[file] {
				continue
			}
			seen[file] = true
			if err := ctx.Err(); err != nil {
				return nil, err
			}
			body, err := readFinalArticleBlock(project, file)
			if err != nil {
				return nil, err
			}
			issues = appendFinalLintIssues(issues, filepath.ToSlash(file), body, rules)
		}
	}
	return issues, nil
}

func finalArticlePaths(articles []model.Article, settings map[string]model.ArticleBuildConfig, status *string) []string {
	statuses := make(map[string]string, len(articles))
	selected := map[string]bool{}
	for _, article := range articles {
		path := filepath.Clean(article.Path)
		statuses[path] = article.Status
		if status == nil || article.Status == *status {
			selected[path] = true
		}
	}
	for id, article := range settings {
		if article.ArticleDir == nil {
			continue
		}
		dir := filepath.Clean(*article.ArticleDir)
		// Some legacy entries are indexed by their named file even though a
		// configured build consumes all Markdown files in the directory.
		namedFile := filepath.Join(dir, id+".md")
		value, ok := statuses[dir]
		if ok {
			delete(selected, namedFile)
		} else {
			value, ok = statuses[namedFile]
		}
		if !ok {
			value = model.ArticleStatusDraft
		}
		if status == nil || value == *status {
			selected[dir] = true
		}
	}
	paths := make([]string, 0, len(selected))
	for path := range selected {
		paths = append(paths, path)
	}
	slices.Sort(paths)
	return paths
}

func finalArticleBlocks(full, relative string) ([]string, error) {
	entries, err := os.ReadDir(full)
	if err != nil {
		return nil, err
	}
	var files []string
	for _, entry := range entries {
		if filepath.Ext(entry.Name()) == ".md" {
			files = append(files, filepath.Join(relative, entry.Name()))
		}
	}
	return files, nil
}

func readFinalArticleBlock(project, relative string) (string, error) {
	path, err := store.CanonicalWithin(project, filepath.Join(project, relative))
	if err != nil {
		return "", err
	}
	info, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", outcome.Usage("article source is not a regular file: "+relative, "")
	}
	body, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if !utf8.Valid(body) {
		return "", fmt.Errorf("stream did not contain valid UTF-8: %s", path)
	}
	return string(body), nil
}

func compileFinalLintRules(raw *yaml.Node) ([]finalLintRule, error) {
	if raw == nil {
		return defaultFinalLintRules, nil
	}
	var config struct {
		Final struct {
			Rules yaml.Node `yaml:"rules"`
		} `yaml:"final"`
	}
	if err := raw.Decode(&config); err != nil {
		failure := outcome.Usage(fmt.Sprintf("invalid article_lint configuration: %s", err), "")
		failure.Cause = err
		return nil, failure
	}
	list := finalRuleNode(&config.Final.Rules)
	if list.Kind == 0 {
		return defaultFinalLintRules, nil
	}
	if list.Kind != yaml.SequenceNode {
		return nil, outcome.Usage("article_lint.final.rules must be a list", "")
	}
	rules := slices.Clone(defaultFinalLintRules)
	for index, node := range list.Content {
		field := fmt.Sprintf("article_lint.final.rules[%d]", index+1)
		node := finalRuleNode(node)
		if node.Kind != yaml.MappingNode {
			return nil, outcome.Usage(field+" must be a mapping", "")
		}
		var rule struct {
			Literal    yaml.Node `yaml:"literal"`
			Pattern    yaml.Node `yaml:"pattern"`
			Suggestion yaml.Node `yaml:"suggestion"`
		}
		if err := node.Decode(&rule); err != nil {
			failure := outcome.Usage(fmt.Sprintf("invalid %s: %s", field, err), "")
			failure.Cause = err
			return nil, failure
		}
		literal, patternNode := finalRuleNode(&rule.Literal), finalRuleNode(&rule.Pattern)
		if (literal.Kind == 0) == (patternNode.Kind == 0) {
			return nil, outcome.Usage(field+" requires exactly one of literal or pattern", "")
		}
		suggestionNode := finalRuleNode(&rule.Suggestion)
		suggestion := strings.TrimSpace(suggestionNode.Value)
		if suggestionNode.Tag != "!!str" || suggestion == "" {
			return nil, outcome.Usage(field+" requires a nonempty string suggestion", "")
		}
		var pattern string
		if literal.Kind != 0 {
			if literal.Tag != "!!str" {
				return nil, outcome.Usage(field+" requires a string literal", "")
			}
			pattern = regexp.QuoteMeta(literal.Value)
		} else {
			if patternNode.Tag != "!!str" {
				return nil, outcome.Usage(field+" requires a string pattern", "")
			}
			pattern = patternNode.Value
		}
		match, err := regexp.Compile(pattern)
		if err != nil {
			failure := outcome.Usage(fmt.Sprintf("%s has an invalid pattern: %s", field, err), "")
			failure.Cause = err
			return nil, failure
		}
		if strings.TrimSpace(pattern) == "" || strings.ContainsAny(pattern, "\r\n") || match.MatchString("") {
			return nil, outcome.Usage(field+" requires a nonempty, single-line match", "")
		}
		rules = append(rules, finalLintRule{match: match, suggestion: suggestion})
	}
	return rules, nil
}

func finalRuleNode(node *yaml.Node) *yaml.Node {
	for node.Kind == yaml.AliasNode {
		node = node.Alias
	}
	return node
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
		line = strings.TrimSuffix(line, "\r")
		for _, rule := range rules {
			for _, match := range rule.match.FindAllString(line, -1) {
				if match != "" {
					issues = append(issues, model.LintIssue{
						Severity: "warning", Kind: "final_leak", Path: path, Line: offset + number,
						Message: fmt.Sprintf("%q: %s", match, rule.suggestion),
					})
				}
			}
		}
	}
	return issues
}
