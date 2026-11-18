package main

import (
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
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
