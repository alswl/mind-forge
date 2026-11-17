package cli

import (
	"fmt"
	"io"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func articleWarnings(stderr io.Writer, warnings []string) error {
	for _, warning := range warnings {
		if _, err := fmt.Fprintln(stderr, "warning: "+warning); err != nil {
			return err
		}
	}
	return nil
}
func articleMtime(seconds uint64) string {
	if seconds == 0 || seconds > 253402300799 {
		return "-"
	}
	return time.Unix(int64(seconds), 0).UTC().Format("2006-01-02 15:04")
}
func articleStatusCell(status string) output.ListCell {
	prefix := "\x1b[2m"
	if status == "published" {
		prefix = "\x1b[32m"
	}
	return output.ListCell{Text: status, ANSIPrefix: prefix}
}

func articleReadCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	all := command.Name() == "list" && !flagPresent(command, "project") && err != nil && strings.HasPrefix(err.Error(), "could not detect current project;")
	if err != nil && !all {
		return err
	}
	if command.Name() == "show" {
		return showArticle(application, project, args[0], stdout, stderr)
	}
	items := []content.ArticleListItem{}
	if all {
		roots, err := content.ArticleProjectRoots(application.RepoRoot)
		if err != nil {
			return err
		}
		for _, root := range roots {
			found, warnings, loadErr := content.ListArticleItems(root)
			if err := articleWarnings(stderr, warnings); err != nil {
				return err
			}
			if loadErr == nil {
				items = append(items, found...)
			}
		}
		sort.SliceStable(items, func(i, j int) bool {
			if items[i].Mtime == items[j].Mtime {
				return items[i].Article.Path < items[j].Article.Path
			}
			return items[i].Mtime > items[j].Mtime
		})
	} else {
		var warnings []string
		items, warnings, err = content.ListArticleItems(project)
		if err != nil {
			return err
		}
		if err := articleWarnings(stderr, warnings); err != nil {
			return err
		}
	}
	if application.Quiet {
		return nil
	}
	data := []map[string]any{}
	view := output.List{Headers: []string{"PATH", "TITLE", "STATUS", "PROMPT", "THINKING", "UPDATED"}, Plural: "articles"}
	if all {
		view.Headers = []string{"PATH", "PROJECT", "TITLE", "STATUS", "UPDATED"}
	}
	for _, item := range items {
		article := item.Article
		if all {
			object, err := jsonObject(article)
			if err != nil {
				return err
			}
			object["mtime"] = item.Mtime
			data = append(data, object)
			relative, err := filepath.Rel(application.RepoRoot, item.Project)
			if err != nil {
				relative = item.Project
			}
			cells := output.TextCells(filepath.ToSlash(filepath.Join(relative, article.Path)), article.Project, article.Title, article.Status, articleMtime(item.Mtime))
			cells[0].Path = true
			cells[3] = articleStatusCell(article.Status)
			view.Rows = append(view.Rows, cells)
		} else {
			data = append(data, item.Data)
			prompt, thought := "-", "-"
			if item.Prompt != nil {
				prompt = "bound"
				if item.Prompt.Mode != nil {
					prompt = *item.Prompt.Mode
				}
				if item.Prompt.BindingStatus == "duplicate" {
					prompt = "duplicate"
				}
			}
			if item.Thinking != nil {
				thought = "yes"
			}
			cells := output.TextCells(article.Path, article.Title, article.Status, prompt, thought, articleMtime(item.Mtime))
			cells[0].Path = true
			cells[2] = articleStatusCell(article.Status)
			view.Rows = append(view.Rows, cells)
		}
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", map[string]any{"articles": data})
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = project
	if all {
		options.RepoRoot = application.RepoRoot
	}
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}

func showArticle(application *app.Context, project, selector string, stdout, stderr io.Writer) error {
	item, warnings, err := content.ShowArticle(project, selector)
	if err != nil {
		return err
	}
	if err := articleWarnings(stderr, warnings); err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return err
	}
	article := item.Article
	block := articleShowBlock(project, config, item)
	if application.Format == "json" {
		object, err := jsonObject(article)
		if err != nil {
			return err
		}
		object["prompt"], object["thinking"] = item.Prompt, item.Thinking
		return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, object))
	}
	return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: project, Hyperlinks: commandProfile(stdout, application).Hyperlinks}))
}

func articleShowBlock(project string, config *model.MindConfig, item content.ArticleListItem) output.ShowBlock {
	article := item.Article
	fields := []output.ShowField{}
	add := func(label, value string, path bool) {
		fields = append(fields, output.ShowField{Label: label, Value: output.ShowValue{Text: value, Path: path}})
	}
	optional := func(label string, value *string, path bool) {
		cell := output.ShowValue{Optional: true, Path: path}
		if value != nil {
			cell.Text = *value
			cell.Present = true
		}
		fields = append(fields, output.ShowField{Label: label, Value: cell})
	}
	add("Path", article.Path, true)
	add("Title", article.Title, false)
	add("Status", article.Status, false)
	add("Content", content.ArticleContentKind(project, article.Path), false)
	add("Article dir", content.EffectiveArticleDir(config, article), true)
	if article.TemplateOrigin != nil {
		add("Template", article.TemplateOrigin.TemplateName+" ("+article.TemplateOrigin.SlotValue+")", false)
	}
	add("Created", article.CreatedAt, false)
	add("Updated", article.UpdatedAt, false)
	if item.Prompt == nil {
		optional("Prompt", nil, false)
	} else {
		add("Prompt", item.Prompt.Path, true)
		optional("Prompt mode", item.Prompt.Mode, false)
		add("Prompt status", item.Prompt.BindingStatus, false)
		if len(item.Prompt.Conflicts) > 0 {
			add("Prompt conflicts", strings.Join(item.Prompt.Conflicts, ", "), false)
		}
	}
	if item.Thinking == nil {
		optional("Thinking", nil, false)
	} else {
		add("Thinking", item.Thinking.Path, true)
	}
	return output.ShowBlock{Kind: "article", Identity: article.Path, Fields: fields}
}
