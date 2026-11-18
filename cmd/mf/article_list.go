package main

import (
	"io"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article list", "article", newArticleListCommand, func(e commandExecution) error {
		return executeArticleList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeArticleList(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	all := !flagPresent(command, "project") && err != nil && strings.HasPrefix(err.Error(), "could not detect current project;")
	if err != nil && !all {
		return err
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
