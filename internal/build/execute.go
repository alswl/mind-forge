package build

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func (p *Prepared) Execute(repo string, runner PipelineRunner) (int64, []string, error) {
	if p.Plan.DryRun {
		return 0, nil, output.Usage("cannot publish a dry-run build plan", "")
	}
	body, warnings, err := p.Render()
	if err != nil {
		return 0, warnings, err
	}
	if err := os.MkdirAll(filepath.Dir(p.Plan.OutputPath), 0755); err != nil {
		return 0, warnings, err
	}
	if err := (store.AtomicWriter{}).Write(p.Plan.OutputPath, []byte(body), 0644); err != nil {
		return 0, warnings, err
	}
	_, notes, err := RunPipeline(p.project, p.assets, p.config.Pipeline, false, runner)
	warnings = append(warnings, notes...)
	if err != nil {
		return 0, warnings, err
	}
	if p.Location != nil {
		if err := content.IndexOutputProjection(repo, p.project); err != nil {
			return 0, warnings, err
		}
	} else if err := p.autoIndex(time.Now()); err != nil {
		warnings = append(warnings, "auto-index failed: "+err.Error())
	}
	return int64(len(body)), warnings, nil
}

func (p *Prepared) autoIndex(now time.Time) error {
	path := filepath.Join(p.project, "mind-index.yaml")
	index, err := store.ReadIndex(path, true)
	if err != nil {
		return err
	}
	source := p.Plan.ArticlePath
	if info, err := os.Stat(source); err == nil && info.IsDir() {
		candidate := filepath.Join(source, p.Plan.Article+".md")
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			source = candidate
		}
	}
	rel, err := filepath.Rel(p.project, source)
	if err != nil {
		return err
	}
	rel = filepath.ToSlash(rel)
	articles := []model.Article{}
	if index.Value.Articles != nil {
		articles = append(articles, (*index.Value.Articles)...)
	}
	for _, article := range articles {
		if article.Path == rel {
			return nil
		}
	}
	timestamp := now.UTC().Format(time.RFC3339)
	articles = append(articles, model.Article{Title: strings.ReplaceAll(p.Plan.Article, "-", " "), Project: filepath.Base(p.project), Type: "blog", Path: rel, Status: "published", CreatedAt: timestamp, UpdatedAt: timestamp})
	updated := index.Value
	updated.Articles = &articles
	return index.Write(path, updated, false)
}
