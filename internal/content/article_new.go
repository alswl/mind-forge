package content

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type ArticleCreateOptions struct {
	Slug                       *string
	Template                   string
	Tags                       []string
	File, Draft, Force, DryRun bool
	Now                        time.Time
}

type ArticleCreateResult struct {
	Filename, Template, Shape, DocsDir string
	Files                              model.JSONArray[string]
	TyporaFrontMatterInjected          bool
	TyporaCopyImagesTo                 *string
}

func typoraAssetsPath(project, assets, fileDir string) (string, error) {
	if filepath.IsAbs(assets) {
		return assets, nil
	}
	target := filepath.Join(project, assets)
	if canonical, err := filepath.EvalSymlinks(target); err == nil {
		target = canonical
	}
	if canonical, err := filepath.EvalSymlinks(fileDir); err == nil {
		fileDir = canonical
	}
	relative, err := filepath.Rel(fileDir, target)
	return filepath.ToSlash(relative), err
}

func CreateArticle(project, title string, options ArticleCreateOptions) (ArticleCreateResult, error) {
	result := ArticleCreateResult{Filename: ArticleFilename(title), Template: options.Template, Shape: "directory"}
	if options.Slug != nil {
		result.Filename = *options.Slug
	}
	if options.File {
		result.Shape = "file"
	}
	if result.Template == "" {
		result.Template = "blank"
	}
	config, err := app.LoadProjectConfig(project, project)
	if err != nil {
		return result, err
	}
	if config == nil {
		defaults := model.DefaultMindConfig()
		config = &defaults
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return result, err
	}
	result.DocsDir = layout.Articles
	docsPath, err := store.CanonicalWithin(project, filepath.Join(project, layout.Articles))
	if err != nil {
		return result, err
	}
	if !options.DryRun {
		if err := os.MkdirAll(docsPath, 0o777); err != nil {
			return result, err
		}
	}
	body, kind, builtin := BuiltinArticleTemplate(result.Template)
	if !builtin {
		if filepath.IsAbs(result.Template) || store.RejectTraversal(result.Template, "project") != nil {
			return result, output.Usage(fmt.Sprintf("template path '%s' is outside the project root", result.Template), "use a path relative to the project root")
		}
		path, err := store.CanonicalWithin(project, filepath.Join(project, result.Template))
		if err != nil {
			return result, err
		}
		raw, err := os.ReadFile(path)
		if os.IsNotExist(err) {
			hint := "built-ins are blank, arch, prd, blog; otherwise expected a path under the project root"
			return result, &output.Error{Kind: "unknown_template", Message: fmt.Sprintf("unknown template '%s': built-ins are blank, arch, prd, blog; otherwise expected a path under the project root", result.Template), Hint: &hint}
		}
		if err != nil {
			return result, err
		}
		if !utf8.Valid(raw) {
			return result, fmt.Errorf("stream did not contain valid UTF-8: %s", path)
		}
		body, kind = string(raw), "blank"
	}
	result.TyporaFrontMatterInjected = config.Plugins == nil || config.Plugins.Typora == nil || config.Plugins.Typora.Enabled == nil || *config.Plugins.Typora.Enabled
	relative := layout.Articles + "/" + result.Filename
	path, err := CheckedPath(project, relative)
	if err != nil {
		return result, err
	}
	alternate, existingShape := path+".md", "file"
	if options.File {
		path, alternate, existingShape = path+".md", path, "directory"
		relative += ".md"
	}
	if _, err := os.Stat(alternate); err == nil {
		hint := "remove the conflicting file or directory manually, or pick a different title"
		return result, &output.Error{Kind: "shape_conflict", Message: fmt.Sprintf("cannot create %s '%s': a %s with the same name already exists; remove it manually or pick a different title", result.Shape, alternate, existingShape), Hint: &hint}
	} else if !os.IsNotExist(err) {
		return result, err
	}
	if _, err := os.Lstat(path); err == nil && !options.Force {
		return result, fileExists(path)
	} else if err != nil && !os.IsNotExist(err) {
		return result, err
	}
	if result.TyporaFrontMatterInjected {
		fileDir := path
		if options.File {
			fileDir = filepath.Dir(path)
		}
		assets, err := typoraAssetsPath(project, layout.Assets, fileDir)
		if err != nil {
			return result, err
		}
		result.TyporaCopyImagesTo = &assets
	}
	now := options.Now
	if now.IsZero() {
		now = time.Now()
	}
	timestamp := now.UTC().Format("2006-01-02T15:04:05Z")
	body = strings.ReplaceAll(strings.ReplaceAll(strings.ReplaceAll(body, "{title}", title), "{created_at}", timestamp), "{tags}", strings.Join(options.Tags, ", "))
	files := map[string][]byte{}
	if options.File {
		result.Files = model.JSONArray[string]{result.Filename + ".md"}
		if result.TyporaCopyImagesTo != nil {
			body = InjectTyporaFrontMatter(body, *result.TyporaCopyImagesTo)
		}
	} else {
		blocks, err := SplitArticleTemplate(body)
		if err != nil {
			return result, err
		}
		result.Files = model.JSONArray[string]{}
		for _, block := range blocks {
			if result.TyporaCopyImagesTo != nil {
				block.Body = InjectTyporaFrontMatter(block.Body, *result.TyporaCopyImagesTo)
			}
			result.Files = append(result.Files, block.Filename)
			files[block.Filename] = []byte(block.Body)
		}
	}
	indexPath := filepath.Join(project, "mind-index.yaml")
	document, err := store.ReadIndex(indexPath, false)
	if err != nil {
		return result, err
	}
	updated := document.Value
	articles := []model.Article{}
	if updated.Articles != nil {
		for _, article := range *updated.Articles {
			if !options.Force || article.Path != relative {
				articles = append(articles, article)
			}
		}
	}
	status := "published"
	if options.Draft {
		status = "draft"
	}
	articles = append(articles, model.Article{Title: title, Project: filepath.Base(project), Type: kind, Path: relative, Status: status, CreatedAt: timestamp, UpdatedAt: timestamp})
	updated.Articles = &articles
	indexRaw, err := document.Render(updated)
	if err != nil {
		return result, err
	}
	if options.DryRun {
		return result, nil
	}
	if err := publishArticle(path, []byte(body), files, options.File, options.Force, func() error { return (store.AtomicWriter{}).Write(indexPath, indexRaw, 0o644) }, func() bool {
		current, err := os.ReadFile(indexPath)
		return err == nil && bytes.Equal(current, indexRaw)
	}); err != nil {
		return result, err
	}
	return result, nil
}

// Keep a replaced author's tree until its index projection has been published.
func publishArticle(path string, body []byte, files map[string][]byte, file, force bool, commit func() error, committed func() bool) (publishError error) {
	staging, err := os.MkdirTemp(filepath.Dir(path), "."+filepath.Base(path)+".create-*")
	if err != nil {
		return err
	}
	retainBackup := false
	defer func() {
		if !retainBackup {
			publishError = errors.Join(publishError, os.RemoveAll(staging))
		}
	}()
	staged, backup := filepath.Join(staging, "content"), filepath.Join(staging, "backup")
	if file {
		err = (store.AtomicWriter{}).Write(staged, body, 0o644)
	} else {
		err = store.CreateDirectory(staged, files)
	}
	if err != nil {
		return err
	}
	hadOriginal := false
	if _, err := os.Lstat(path); err == nil {
		if !force {
			return fileExists(path)
		}
		if err := os.Rename(path, backup); err != nil {
			return err
		}
		hadOriginal = true
	} else if !os.IsNotExist(err) {
		return err
	}
	restore := func() error {
		if err := os.RemoveAll(path); err != nil {
			retainBackup = hadOriginal
			return err
		}
		if hadOriginal {
			if err := os.Rename(backup, path); err != nil {
				retainBackup = true
				return fmt.Errorf("restore author content from %s: %w", backup, err)
			}
		}
		return nil
	}
	if err := os.Rename(staged, path); err != nil {
		return errors.Join(err, restore())
	}
	if err := commit(); err != nil {
		// Atomic publication can succeed before directory fsync reports failure.
		if committed() {
			return err
		}
		return errors.Join(err, restore())
	}
	return (store.OSFilesystem{}).SyncDir(filepath.Dir(path))
}
