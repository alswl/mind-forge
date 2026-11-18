package source

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type Service struct {
	Now        func() time.Time
	Acquire    func(context.Context, string, AcquisitionOptions) ([]byte, error)
	WriteIndex func(string, []byte) error
}

func (service Service) timestamp() string {
	now := time.Now()
	if service.Now != nil {
		now = service.Now()
	}
	return now.UTC().Format(time.RFC3339)
}

func projectLayout(project string) (model.EffectiveLayout, error) {
	config, err := app.LoadProjectConfig(project, project)
	if err != nil {
		return model.EffectiveLayout{}, err
	}
	if config == nil {
		value := model.DefaultMindConfig()
		config = &value
	}
	layout, _ := app.ResolveEffectiveLayout(config)
	return layout, nil
}

func readIndex(project string) (*store.IndexDocument, error) {
	return store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
}

func sourceEntries(index model.Index) []model.Source {
	if index.Sources == nil {
		return nil
	}
	return append([]model.Source(nil), (*index.Sources)...)
}

func (service Service) save(project string, document *store.IndexDocument, updated model.Index) (bool, error) {
	raw, err := document.Render(updated)
	if err != nil {
		return false, err
	}
	name := filepath.Join(project, "mind-index.yaml")
	if service.WriteIndex != nil {
		err = service.WriteIndex(name, raw)
	} else {
		err = (store.AtomicWriter{}).Write(name, raw, 0644)
	}
	if err == nil {
		return true, nil
	}
	actual, readError := os.ReadFile(name)
	return readError == nil && bytes.Equal(actual, raw), err
}

func List(project string, filter, kind *string) ([]model.Source, error) {
	document, err := readIndex(project)
	if err != nil {
		return nil, err
	}
	result := make([]model.Source, 0)
	for _, item := range sourceEntries(document.Value) {
		if filter != nil && !strings.Contains(textcase.Lower(item.Name), textcase.Lower(*filter)) {
			continue
		}
		if kind != nil && item.Kind != *kind {
			continue
		}
		result = append(result, item)
	}
	sort.SliceStable(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, nil
}

func ResolveInput(input, project, sources, cwd, root string) (string, error) {
	exists := func(name string) bool { _, err := os.Stat(name); return err == nil }
	if filepath.IsAbs(input) {
		if exists(input) {
			return input, nil
		}
		return "", output.Usage(fmt.Sprintf("file not found\n  input: %s\n  resolved: %s", input, input), "pass an existing file path")
	}
	tried := []string{}
	for _, anchor := range []string{project, sources, root, cwd} {
		candidate := filepath.Join(anchor, input)
		if exists(candidate) {
			return candidate, nil
		}
		tried = append(tried, candidate)
	}
	return "", output.Usage("file not found\n  input: "+input+"\n  tried:\n    "+strings.Join(tried, "\n    "), "pass a path relative to the project, its sources/ dir, the repo root, or an absolute path")
}

func notFound(selector string) error {
	hint := "use `mf source list` to see available sources"
	return &output.Error{Kind: "not_found", Message: "source '" + selector + "' not found", Hint: &hint}
}

func fileExists(name string) error {
	hint := "pass --force to overwrite"
	return &output.Error{Kind: "file_exists", Message: "refusing to overwrite existing file: " + name, Hint: &hint}
}

func relativePath(project, name string) (string, error) {
	canonical, err := filepath.EvalSymlinks(project)
	if err != nil {
		return "", err
	}
	parent, err := store.CanonicalWithin(project, filepath.Dir(name))
	if err != nil {
		return "", err
	}
	rel, err := filepath.Rel(canonical, filepath.Join(parent, filepath.Base(name)))
	return strings.ReplaceAll(filepath.ToSlash(rel), "\\", "/"), err
}

func (service Service) Update(project, name string, rename, url *string) (model.Source, error) {
	if rename == nil && url == nil {
		return model.Source{}, output.Usage("nothing to update: use --rename or --url", "pass --rename <NAME> or --url <URL> to modify the source")
	}
	document, err := readIndex(project)
	if err != nil {
		return model.Source{}, err
	}
	items := sourceEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Name == name {
			pos = i
			break
		}
	}
	if pos < 0 {
		return model.Source{}, output.Usage("source '"+name+"' not found", "use `mf source list` to see available sources")
	}
	if rename != nil && *rename != name {
		for _, item := range items {
			if item.Name == *rename {
				return model.Source{}, output.Usage("a source named '"+*rename+"' already exists", "use a different --rename value")
			}
		}
	}
	if url != nil {
		if err := ValidateURL(*url); err != nil {
			return model.Source{}, err
		}
	}
	if rename != nil {
		items[pos].Name = *rename
	}
	if url != nil {
		items[pos].URL = url
	}
	items[pos].UpdatedAt = service.timestamp()
	result := items[pos]
	sort.SliceStable(items, func(i, j int) bool { return items[i].Name < items[j].Name })
	updated := document.Value
	updated.Sources = &items
	_, err = service.save(project, document, updated)
	return result, err
}
