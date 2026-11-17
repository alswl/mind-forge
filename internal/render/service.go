package render

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

type Template struct {
	Info model.RenderTemplate
	Body string
}
type Content struct {
	Path, Body string
	Size       uint64
}

func Templates(root string) ([]Template, error) {
	items := []Template{
		{Info: model.RenderTemplate{Name: "report", Label: "Work report", Description: "Work/reporting presentation", Source: "built_in", Default: true}, Body: "Render the supplied content as an HTML report page for work presentation.\nPrioritize an executive summary, key conclusions, progress highlights, risks, and next steps.\nUse scannable HTML sections with clear headings."},
		{Info: model.RenderTemplate{Name: "paper", Label: "Paper", Description: "Academic/paper-style presentation", Source: "built_in"}, Body: "Render the supplied content as an academic/paper-style HTML page.\nInclude a title, abstract, sectioned argument with numbered sections, citation placeholders where appropriate, and a conclusion."},
	}
	dir := filepath.Join(root, ".mind-forge/renders")
	entries, err := os.ReadDir(dir)
	if os.IsNotExist(err) {
		return items, nil
	}
	if err != nil {
		return nil, err
	}
	for _, entry := range entries {
		path := filepath.Join(dir, entry.Name())
		info, err := os.Stat(path)
		if err != nil {
			return nil, err
		}
		if !info.Mode().IsRegular() || filepath.Ext(entry.Name()) != ".md" {
			continue
		}
		name := strings.TrimSuffix(entry.Name(), ".md")
		if name == "" || strings.ContainsAny(name, "/\\") {
			continue
		}
		text, err := readText(path)
		if err != nil {
			return nil, err
		}
		relative := ".mind-forge/renders/" + entry.Name()
		if strings.TrimSpace(text) == "" {
			return nil, output.Usage(fmt.Sprintf("custom render template '%s' is empty: %s", name, relative), "add Markdown guidance to the template file or remove it")
		}
		if name == "report" || name == "paper" {
			return nil, output.Usage(fmt.Sprintf("custom template '%s' at '%s' conflicts with built-in template name; rename the file", name, relative), "built-in template names are: report, paper")
		}
		label, description, body := ParseTemplate(text, name)
		items = append(items, Template{Info: model.RenderTemplate{Name: name, Label: label, Description: description, Source: "custom", Path: &relative}, Body: body})
	}
	return items, nil
}

// ParseTemplate retains the historical line-based metadata parser, including
// literal fallback for an unclosed delimiter. Guidance is never YAML-decoded.
func ParseTemplate(text, fallback string) (label, description, body string) {
	label, body = fallback, text
	trimmed := strings.TrimLeftFunc(text, unicode.IsSpace)
	if !strings.HasPrefix(trimmed, "---") {
		return
	}
	end := strings.Index(trimmed[3:], "\n---")
	if end < 0 {
		return
	}
	front := trimmed[3 : 3+end]
	body = strings.TrimLeftFunc(trimmed[3+end+4:], unicode.IsSpace)
	for _, line := range strings.Split(front, "\n") {
		if value, ok := strings.CutPrefix(line, "label:"); ok {
			label = strings.Trim(strings.TrimSpace(value), "\"")
		}
		if value, ok := strings.CutPrefix(line, "description:"); ok {
			description = strings.Trim(strings.TrimSpace(value), "\"")
		}
	}
	return
}

func readText(path string) (string, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if !utf8.Valid(raw) {
		return "", fmt.Errorf("%s: stream did not contain valid UTF-8", path)
	}
	return string(raw), nil
}

func missing(message, hint string) error {
	return &output.Error{Kind: "not_found", Message: message, Hint: &hint}
}
func ResolveOutput(project, article, dir, format string) (Content, error) {
	path := filepath.Join(project, dir, article+"."+format)
	info, err := os.Stat(path)
	if os.IsNotExist(err) {
		return Content{}, missing(fmt.Sprintf("output not found for article '%s'", article), fmt.Sprintf("run `mf build %s` first", article))
	}
	if err != nil {
		return Content{}, err
	}
	if info.Size() == 0 {
		return Content{}, missing(fmt.Sprintf("output for article '%s' is empty", article), fmt.Sprintf("run `mf build %s` to regenerate it", article))
	}
	body, err := readText(path)
	if err != nil {
		return Content{}, err
	}
	if strings.TrimSpace(body) == "" {
		return Content{}, missing(fmt.Sprintf("output for article '%s' contains only whitespace", article), fmt.Sprintf("run `mf build %s` to regenerate it", article))
	}
	relative, err := filepath.Rel(project, path)
	if err != nil || relative == ".." || strings.HasPrefix(relative, "../") {
		relative = path
	}
	return Content{Path: filepath.ToSlash(relative), Body: body, Size: uint64(info.Size())}, nil
}

func discoverOutputs(project, dir, format string) ([]Content, error) {
	path := filepath.Join(project, dir)
	info, err := os.Stat(path)
	if os.IsNotExist(err) || err == nil && !info.IsDir() {
		return nil, missing(fmt.Sprintf("output directory '%s' does not exist", path), "run `mf build` for the desired articles first")
	}
	if err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(path)
	if err != nil {
		return nil, err
	}
	results := []Content{}
	for _, entry := range entries {
		file := filepath.Join(path, entry.Name())
		info, err := os.Stat(file)
		if err != nil {
			return nil, err
		}
		if !info.Mode().IsRegular() || filepath.Ext(file) != "."+format || info.Size() == 0 {
			continue
		}
		body, err := readText(file)
		if err != nil {
			return nil, err
		}
		if strings.TrimSpace(body) == "" {
			continue
		}
		rel, err := filepath.Rel(project, file)
		if err != nil {
			rel = file
		}
		results = append(results, Content{Path: filepath.ToSlash(rel), Body: body, Size: uint64(info.Size())})
	}
	if len(results) == 0 {
		return nil, missing(fmt.Sprintf("no renderable output found in '%s'", path), "run `mf build` for the desired articles first")
	}
	return results, nil
}

func AssemblePrompt(guidance string, outputs []Content, form string) string {
	var result strings.Builder
	result.WriteString("You are an Agent rendering existing mf output into HTML.\n\nTask:\n" + guidance)
	if form == "document" {
		result.WriteString("\n\nRender as a complete HTML document with <html>, <head>, and <body> tags.")
	}
	if form == "fragment" {
		result.WriteString("\n\nRender as an HTML fragment that can be embedded in an existing page (no <html>, <head>, or <body> tags).")
	}
	result.WriteString("\n\nmf is NOT rendering the final HTML. You, the Agent, are responsible for producing the HTML output.\nPreserve the source meaning, including existing HTML, code blocks, tables, image references, and links; convert structure only when it improves the HTML presentation without losing semantics.\n")
	for _, item := range outputs {
		result.WriteString("\n--- BEGIN MF OUTPUT: " + item.Path + " ---\n" + item.Body)
		if !strings.HasSuffix(item.Body, "\n") {
			result.WriteByte('\n')
		}
		result.WriteString("--- END MF OUTPUT: " + item.Path + " ---\n")
	}
	return result.String()
}

func validateTemplateName(name string) error {
	if name == "" {
		return output.Usage("template name cannot be empty", "use `mf render template list` to see available templates")
	}
	if strings.ContainsAny(name, "/\\") {
		return output.Usage("invalid template name: '"+name+"'", "template names must not contain path separators")
	}
	return nil
}

func Generate(project, name, dir, format string, article *string, templateName, form string, templates []Template) (model.GeneratedPrompt, error) {
	result := model.GeneratedPrompt{Template: templateName, Project: name, Article: article, Scope: "project", Outputs: []model.OutputInfo{}}
	if err := validateTemplateName(templateName); err != nil {
		return result, err
	}

	var selected *Template
	for i := range templates {
		if templates[i].Info.Name == templateName {
			selected = &templates[i]
			break
		}
	}
	if selected == nil {
		return result, output.Usage("unknown template: '"+templateName+"'", "use `mf render template list` to see available templates")
	}
	var outputs []Content
	var err error
	if article != nil {
		if *article == "" {
			return result, output.Usage("article name cannot be empty", "provide a non-empty article name, or omit it for project-scope render")
		}
		result.Scope = "article"
		var item Content
		item, err = ResolveOutput(project, *article, dir, format)
		outputs = []Content{item}
	} else {
		outputs, err = discoverOutputs(project, dir, format)
	}
	if err != nil {
		return result, err
	}
	result.TemplateSource = selected.Info.Source
	result.Prompt = AssemblePrompt(selected.Body, outputs, form)
	for _, item := range outputs {
		result.Outputs = append(result.Outputs, model.OutputInfo{Path: item.Path, SizeBytes: item.Size})
	}
	return result, nil
}
