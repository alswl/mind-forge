package publish

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/task"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/services/content"
)

type RunOptions struct {
	Context                                          context.Context
	Target                                           *string
	DryRun, Force, StripFrontMatter, KeepFrontMatter bool
}
type UpdateOptions struct {
	Target      string
	Status, URL *string
	DryRun      bool
	Now         time.Time
}

func articleFor(root, project, selector string, config *model.MindConfig) (model.Article, *model.Output, error) {
	if config.ContentLayout != nil {
		selected, err := content.ResolveOutput(root, project, selector)
		if err != nil {
			return model.Article{}, nil, err
		}
		return model.Article{Title: selected.Name, Project: filepath.Base(project), Path: selected.Path, Type: "blog", Status: "draft"}, &selected, nil
	}
	article, err := content.ResolveIndexedArticle(project, selector)
	return article, nil, err
}

func LocateArtifact(project string, config *model.MindConfig, article model.Article, selected *model.Output) (string, uint64, error) {
	var path string
	var err error
	if selected != nil {
		path, err = content.CheckedPath(project, selected.BuildPath)
		if err != nil {
			return "", 0, err
		}
	} else if article.TemplateOrigin != nil {
		path = joined(project, article.Path)
	} else {
		layout, err := app.ResolveAndValidateLayout(project, config)
		if err != nil {
			return "", 0, err
		}
		format := config.Build.Format
		if format == "" {
			format = "md"
		}
		path = joined(joined(project, layout.BuildOutput), content.ArticleOutputStem(article.Path)+"."+format)
	}
	info, err := os.Stat(path)
	if err != nil {
		if selected != nil {
			return "", 0, problem("build_artifact_missing", "build artifact not found: "+path, "build the selected output first")
		}
		if article.TemplateOrigin != nil {
			return "", 0, problem("build_artifact_missing", "build artifact (template source) missing at "+path, "the template-matched file was removed; regenerate it or remove the entry from mind-index.yaml")
		}
		if _, err := os.Stat(joined(project, article.Path)); os.IsNotExist(err) {
			return "", 0, problem("no_article_files", "no article files for '"+article.Title+"'", "create the article file or remove the declaration from mind.yaml")
		}
		return "", 0, problem("build_artifact_missing", "build artifact not found: "+path, "run `mf build "+content.ArticleOutputStem(article.Path)+"` first")
	}
	return path, uint64(info.Size()), nil
}

func promptDestination(project string, config *model.MindConfig, target model.PublishTarget, article model.Article, selected *model.Output) (string, error) {
	override := configString(target, "out_path")
	if selected != nil {
		if override == "" {
			override = "build/" + selected.Mind + "/" + selected.Name + ".yuque.md"
		}
		if !strings.HasPrefix(override, "build/") {
			return "", output.Usage("publish artifact must stay under project build/", "")
		}
		return content.CheckedPath(project, override)
	}
	if strings.TrimSpace(override) != "" {
		path, err := store.CanonicalWithin(project, joined(project, override))
		if err != nil {
			return "", output.Usage("out_path '"+override+"' resolves outside the project", "use a project-relative path")
		}
		return path, nil
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return "", err
	}
	format := config.Build.Format
	if format == "" {
		format = "md"
	}
	return joined(joined(project, layout.BuildOutput), content.ArticleOutputStem(article.Path)+".yuque."+format), nil
}

func publishFile(path, body string, force bool) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	if _, err := os.Stat(path); err == nil && !force {
		return problem("file_exists", "file already exists: "+path, "use --force to overwrite")
	}
	return (store.AtomicWriter{}).Write(path, []byte(body), 0644)
}

func Run(root, project, selector string, opts RunOptions) (model.PublishRunOutcome, *model.Output, []string, error) {
	var result model.PublishRunOutcome
	ctx := task.Context(opts.Context)
	if err := ctx.Err(); err != nil {
		return result, nil, nil, err
	}
	if selector == "" || strings.Contains(selector, "\\") {
		return result, nil, nil, output.Usage("invalid article name: '"+selector+"'", "article must be kebab-case or template-name/slot-value")
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return result, nil, nil, err
	}
	article, selected, err := articleFor(root, project, selector, config)
	if err != nil && config.ContentLayout == nil {
		if opts.DryRun {
			article, err = content.PreviewRefreshedArticle(project, selector, config)
		} else {
			if _, _, refreshErr := content.IndexArticles(project, false, time.Now()); refreshErr != nil {
				return result, nil, nil, refreshErr
			}
			article, selected, err = articleFor(root, project, selector, config)
		}

		if err != nil {
			err = problem("not_found", "article '"+selector+"' not found in mind-index.yaml", "run `mf article index` to refresh the index")
		}
	}
	if err != nil {
		return result, nil, nil, err
	}
	resolved, err := ResolveTarget(root, project, config, opts.Target)
	if err != nil {
		return result, selected, nil, err
	}
	target := resolved.Target
	if target.Type != "local" && target.Type != "yuque-prompt" {
		return result, selected, nil, output.NotImplemented("publish target type '"+target.Type+"' is not yet implemented", "tracked in upcoming ROADMAP-004; use type 'local' or 'yuque-prompt' instead")
	}
	artifact, size, err := LocateArtifact(project, config, article, selected)
	if err != nil {
		return result, selected, nil, err
	}
	notes := []string{}
	if target.Type == "local" {
		dir, date, err := LocalPath(resolved.Base, target, article)
		if err != nil {
			return result, selected, nil, err
		}
		format := config.Build.Format
		if format == "" {
			format = "md"
		}
		prefix := ""
		if target.Prefix != nil {
			prefix = *target.Prefix
		}
		stem := strings.TrimSuffix(selector[strings.LastIndex(selector, "/")+1:], ".md")
		dest := joined(dir, prefix+stem+"."+format)
		if selected != nil {
			dest = joined(joined(dir, selected.Mind), prefix+selected.Name+".md")
			canonical, err := store.CanonicalWithin(string(filepath.Separator), dest)
			if err != nil {
				return result, selected, nil, err
			}
			projectRoot, err := filepath.EvalSymlinks(project)
			if err != nil {
				return result, selected, nil, err
			}
			rel, err := filepath.Rel(projectRoot, canonical)
			if err != nil {
				return result, selected, nil, err
			}
			if rel != ".." && !strings.HasPrefix(rel, "../") {
				if !strings.HasPrefix(filepath.ToSlash(rel), "build/") {
					return result, selected, nil, output.Usage("local publish destination inside a mind project must be under build/", "")
				}
				if _, err := content.CheckedPath(project, filepath.ToSlash(rel)); err != nil {
					return result, selected, nil, err
				}
			}
		}
		if size == 0 {
			notes = append(notes, "build artifact is empty")
		}
		if err := ctx.Err(); err != nil {
			return result, selected, notes, err
		}
		if !opts.DryRun {
			raw, err := readText(artifact)
			if err != nil {
				return result, selected, notes, err
			}
			if err := publishFile(dest, PrepareContent(raw, config, article, opts.StripFrontMatter, opts.KeepFrontMatter), opts.Force); err != nil {
				return result, selected, notes, err
			}
		}
		result.Local = &model.LocalRunOutcome{TargetName: target.Name, Article: selector, Source: artifact, Destination: dest, SizeBytes: size, DryRun: opts.DryRun, EffectiveDate: date}
		return result, selected, notes, nil
	}
	raw, err := readText(artifact)
	if err != nil {
		return result, selected, nil, err
	}
	if err := ctx.Err(); err != nil {
		return result, selected, nil, err
	}
	raw = PrepareContent(raw, config, article, opts.StripFrontMatter, opts.KeepFrontMatter)
	transformed, transforms, err := Transforms(raw, filepath.Dir(artifact), target, project)
	if err != nil {
		return result, selected, nil, err
	}
	dest, err := promptDestination(project, config, target, article, selected)
	if err != nil {
		return result, selected, nil, err
	}
	suggested := fmt.Sprintf("mf publish update %s --target %s --status published --target-url <URL>", selector, target.Name)
	prompt := fmt.Sprintf("Please publish the following article to yuque-prompt (target: %s).\nArticle: %s\nSource: %s\nAfter publishing, run:\n\n    %s", target.Name, selector, article.Path, suggested)
	envelope := target.Config
	if envelope == nil {
		envelope = map[string]any{}
	}
	outcome := &model.YuquePromptRunOutcome{TargetName: target.Name, Article: selector, ArticlePath: article.Path, BuildArtifactPath: artifact, Content: transformed, Prompt: prompt, Envelope: envelope, SuggestedUpdateCommand: suggested, DryRun: opts.DryRun, Transforms: transforms}
	if raw == "" {
		notes = append(notes, "build artifact is empty")
	}
	for _, missing := range transforms.SVGPNGMissing {
		notes = append(notes, "no .png counterpart found for '"+missing+"'; kept original .svg reference")
	}
	if err := ctx.Err(); err != nil {
		return result, selected, notes, err
	}
	if !opts.DryRun {
		if err := publishFile(dest, transformed, opts.Force); err != nil {
			return result, selected, notes, err
		}
		outcome.Destination = &dest
	}
	result.YuquePrompt = outcome
	return result, selected, notes, nil
}

func Update(root, project, selector string, opts UpdateOptions) (model.PublishUpdateOutcome, *model.Output, error) {
	result := model.PublishUpdateOutcome{Article: selector, TargetName: opts.Target, DryRun: opts.DryRun}
	if selector == "" || strings.Contains(selector, "\\") {
		return result, nil, output.Usage("invalid article name: '"+selector+"'", "provide an article slug or indexed article path")
	}
	if opts.Status == nil && opts.URL == nil {
		return result, nil, output.Usage("--status and --target-url cannot both be omitted", "provide at least one of --status or --target-url")
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return result, nil, err
	}
	path := filepath.Join(project, "mind-index.yaml")
	index, err := store.ReadIndex(path, true)
	if err != nil {
		return result, nil, err
	}
	article, selected, err := articleFor(root, project, selector, config)
	if err != nil {
		return result, nil, err
	}
	found := false
	if config.Publish.Targets != nil {
		for _, target := range *config.Publish.Targets {
			found = found || target.Name == opts.Target
		}
	}
	if !found {
		return result, selected, problem("not_found", "publish target '"+opts.Target+"' not found in mind.yaml", "check `publish.targets[].name` in mind.yaml")
	}
	records := []model.PublishRecord{}
	if index.Value.PublishRecords != nil {
		records = append(records, (*index.Value.PublishRecords)...)
	}
	pos := -1
	var existing *model.PublishRecord
	for n := range records {
		if records[n].Path == article.Path && records[n].TargetName == opts.Target {
			pos = n
			existing = &records[n]
			break
		}
	}
	now := opts.Now
	if now.IsZero() {
		now = time.Now()
	}
	result.Record, result.Action, err = Upsert(existing, article.Path, opts.Target, opts.Status, opts.URL, now)
	if err != nil {
		return result, selected, err
	}
	if !opts.DryRun {
		if pos < 0 {
			records = append(records, result.Record)
		} else {
			records[pos] = result.Record
		}
		updated := index.Value
		updated.PublishRecords = &records
		if err := index.Write(path, updated, false); err != nil {
			return result, selected, err
		}
	}
	return result, selected, nil
}
