package build

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/task"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/services/content"
)

type Options struct {
	ProcessIO ProcessIO
	Context   context.Context
	Source    *string
	Out       *string
	Format    string
	DryRun    bool
}

type InputFile struct {
	Path string `json:"path"`
	Size int64  `json:"size"`
}

type BannerInfo struct {
	Enabled bool    `json:"enabled"`
	Level   *string `json:"level"`
	Text    *string `json:"text"`
}

type Plan struct {
	Article       string          `json:"article"`
	Format        string          `json:"format"`
	Project       string          `json:"project"`
	ArticlePath   string          `json:"article_path"`
	InputFiles    []InputFile     `json:"input_files"`
	MergeOrder    []string        `json:"merge_order"`
	OutputPath    string          `json:"output_path"`
	SizeBytes     int64           `json:"size_bytes"`
	EstimatedSize int64           `json:"estimated_size"`
	DryRun        bool            `json:"dry_run"`
	Banner        *BannerInfo     `json:"banner"`
	StripFirstH1  bool            `json:"strip_first_h1"`
	Pipeline      []PipelineStage `json:"pipeline"`
}

type Prepared struct {
	processIO ProcessIO
	Context   context.Context
	Selector  string
	Plan      Plan
	Location  *model.Output
	project   string
	files     []string
	sourceDir string
	assets    string
	config    model.BuildConfig
	banner    string
	template  string
}

func joined(root, path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(root, path)
}

func Prepare(repo, project, selector string, opts Options) (*Prepared, error) {
	task.Notify(opts.Context, task.Stage{Operation: "build", Name: "preparing inputs"})
	if err := task.Context(opts.Context).Err(); err != nil {
		return nil, err
	}
	config, err := app.LoadProjectConfig(project, repo)
	if err != nil {
		return nil, err
	}
	if config == nil {
		return nil, output.Usage("project missing mind.yaml", "run `mf config init` to create one")
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return nil, err
	}
	if err := ValidatePipeline(config.Build.Pipeline); err != nil {
		return nil, err
	}
	if config.Build.Banner != nil && strings.TrimSpace(config.Build.Banner.Text) == "" {
		return nil, output.Usage("build.banner.text must not be empty", "")
	}
	format := opts.Format
	if format == "" {
		format = "markdown"
	}
	if format != "markdown" && format != "html" {
		return nil, output.Usage("invalid build format: "+format, "")
	}
	ext := "md"
	if format == "html" {
		ext = "html"
	}
	p := &Prepared{processIO: opts.ProcessIO, Context: opts.Context, project: project, config: config.Build, assets: joined(project, layout.Assets)}
	p.Plan = Plan{Article: selector, Project: filepath.Base(project), Format: format, DryRun: opts.DryRun, StripFirstH1: config.Build.StripFirstH1, InputFiles: []InputFile{}, Pipeline: []PipelineStage{}}
	if format == "html" {
		p.template, err = loadHTMLTemplate(project, config.Build.HTMLTemplate)
		if err != nil {
			return nil, err
		}
	}
	if opts.Source != nil {
		p.Plan.ArticlePath = *opts.Source
	}
	if config.ContentLayout != nil {
		selected := selector
		if opts.Source != nil {
			selected, err = filepath.Rel(project, *opts.Source)
			if err != nil {
				return nil, err
			}
			selected = filepath.ToSlash(selected)
		}
		location, err := content.ResolveOutput(repo, project, selected)
		if err != nil {
			return nil, err
		}
		p.Location = &location
		p.Plan.ArticlePath, err = content.CheckedPath(project, location.Path)
		if err != nil {
			return nil, err
		}
		if opts.Source == nil {
			p.Plan.Article = location.Identity
		}
		p.Plan.OutputPath = joined(project, strings.TrimSuffix(location.BuildPath, filepath.Ext(location.BuildPath))+"."+ext)
		if *config.ContentLayout == "bundle" {
			p.assets, err = content.CheckedPath(project, location.Mind+"/assets")
		} else {
			p.assets, err = content.CheckedPath(project, "assets/"+location.Mind)
		}
		if err != nil {
			return nil, err
		}
	} else {
		if opts.Source == nil {
			if article, ok := config.Build.Articles[selector]; ok && article.ArticleDir != nil {
				p.Plan.ArticlePath = joined(project, *article.ArticleDir)
				info, err := os.Stat(p.Plan.ArticlePath)
				if err != nil || !info.IsDir() {
					return nil, output.Usage(fmt.Sprintf("configured article_dir '%s' for article '%s' does not exist or is not a directory", *article.ArticleDir, selector), "check the path or create the directory")
				}
			} else {
				article, err := content.ResolveIndexedArticle(project, selector)
				if err != nil {
					return nil, err
				}
				p.Plan.ArticlePath = joined(project, article.Path)
			}
		}
		p.Plan.OutputPath = joined(joined(project, layout.BuildOutput), content.ArticleOutputStem(selector)+"."+ext)
	}
	if opts.Out != nil {
		p.Plan.OutputPath = *opts.Out
	}
	if p.Location != nil {
		rel, err := filepath.Rel(project, p.Plan.OutputPath)
		if err != nil || !strings.HasPrefix(filepath.ToSlash(rel), "build/") {
			return nil, output.Usage("Mind Output build artifacts must stay under the project build directory", "")
		}
		p.Plan.OutputPath, err = content.CheckedPath(project, filepath.ToSlash(rel))
		if err != nil {
			return nil, err
		}
	}
	p.Plan.OutputPath, err = store.CanonicalWithin(string(filepath.Separator), p.Plan.OutputPath)
	if err != nil {
		return nil, err
	}
	source, err := filepath.EvalSymlinks(p.Plan.ArticlePath)
	if err != nil {
		return nil, output.Usage("article path not found: "+p.Plan.ArticlePath, "the file or directory may have been moved or deleted")
	}
	info, err := os.Stat(source)
	if err != nil {
		return nil, err
	}
	rel, err := filepath.Rel(source, p.Plan.OutputPath)
	if err != nil {
		return nil, err
	}
	if rel == "." || info.IsDir() && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return nil, output.Usage("build output overlaps the article source", "")
	}
	p.sourceDir = filepath.Dir(source)
	if info.IsDir() {
		p.sourceDir = source
		entries, err := os.ReadDir(source)
		if err != nil {
			return nil, err
		}
		for _, entry := range entries {
			if filepath.Ext(entry.Name()) == ".md" {
				p.files = append(p.files, filepath.Join(source, entry.Name()))
			}
		}
	} else {
		p.files = []string{source}
	}
	if len(p.files) == 0 {
		return nil, output.Usage("no markdown files found in "+p.Plan.ArticlePath, "")
	}
	for _, file := range p.files {
		if err := task.Context(opts.Context).Err(); err != nil {
			return nil, err
		}
		info, err := os.Stat(file)
		if err != nil {
			return nil, err
		}
		rel, err := filepath.Rel(project, file)
		if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
			rel = file
		}
		p.Plan.InputFiles = append(p.Plan.InputFiles, InputFile{Path: filepath.ToSlash(rel), Size: info.Size()})
		p.Plan.SizeBytes += info.Size()
	}
	p.Plan.MergeOrder = append([]string{}, config.Build.MergeOrder...)
	if len(p.Plan.MergeOrder) == 0 {
		p.Plan.MergeOrder = []string{p.Plan.Article}
	}
	if banner := config.Build.Banner; banner != nil {
		p.Plan.Banner = &BannerInfo{Enabled: true, Level: banner.Level, Text: &banner.Text}
		p.banner = strings.TrimSpace(banner.Text) + "\n\n"
		if banner.Level != nil {
			p.banner = ":::" + *banner.Level + "\n" + strings.TrimSpace(banner.Text) + "\n:::\n\n"
		}
	}
	p.Plan.EstimatedSize = p.Plan.SizeBytes + int64(len(p.banner))
	if opts.DryRun {
		p.Plan.Pipeline, _, err = RunPipelineContext(task.Context(opts.Context), project, p.assets, config.Build.Pipeline, true, nil)
		if err != nil {
			return nil, err
		}
	}
	return p, nil
}

func (p *Prepared) Render() (string, []string, error) {
	task.Notify(p.Context, task.Stage{Operation: "build", Name: "assembling content"})
	blocks := make([]Block, 0, len(p.files))
	for i, path := range p.files {
		if err := task.Context(p.Context).Err(); err != nil {
			return "", nil, err
		}
		body, err := readText(path)
		if err != nil {
			return "", nil, err
		}
		blocks = append(blocks, Block{Path: p.Plan.InputFiles[i].Path, Content: body})
	}
	body, warnings, err := AssembleBlocks(blocks, false, "")
	if err != nil {
		return "", nil, output.Usage(err.Error(), "")
	}
	outDir := filepath.Dir(p.Plan.OutputPath)
	body = rewriteRelativePaths(body, p.sourceDir, outDir, &warnings)

	if p.Plan.StripFirstH1 {
		body = stripFirstH1(body)
	}
	if p.banner != "" {
		body = insertBanner(body, p.banner)
	}
	if err := task.Context(p.Context).Err(); err != nil {
		return "", warnings, err
	}
	if p.Plan.Format == "html" {
		body, err = RenderHTML(body, p.Plan.Article, p.template)
	}
	return body, warnings, err
}
