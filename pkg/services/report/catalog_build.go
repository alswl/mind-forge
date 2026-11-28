package report

import (
	"context"
	"fmt"
	"path/filepath"
)

func (service Service) BuildCatalog(ctx context.Context, scope Scope, slug, theme string) (Result, error) {
	root, err := scope.openReport(slug)
	if err != nil {
		return Result{}, err
	}
	defer root.Close()
	if theme == "" {
		theme = "system"
	}
	if !validTheme(theme) {
		return Result{}, fmt.Errorf("%w: unsupported theme %q", ErrInvalid, theme)
	}
	const artifact = "outputs/catalog.html"
	if err := checkArtifact(root, artifact); err != nil {
		return Result{}, fmt.Errorf("%w: %s: %w", ErrInvalid, artifact, err)
	}
	catalog, err := service.Catalog(Component, "")
	if err != nil {
		return Result{}, err
	}
	items := make([]CatalogComponent, 0, len(catalog.Items))
	for _, item := range catalog.Items {
		if _, err := catalog.lookup(Component, item.ID); err != nil {
			return Result{}, err
		}
		item = service.resolveEntry(item)
		example := item.Example
		if item.Source == "repo" {
			example = filepath.Join(service.Registry, example)
		}
		items = append(items, CatalogComponent{
			Module:      Module{ID: item.ID, Version: item.Version, Entry: item.Entry, Schema: item.Schema},
			Description: item.Description, Example: example,
		})
	}
	if len(catalog.Diagnostics) != 0 {
		first := catalog.Diagnostics[0]
		return Result{}, fmt.Errorf("%w: %s: %s", ErrInvalid, first.Path, first.Message)
	}
	const id = "builtin/catalog"
	const version = "1.0.0"
	result := Result{Protocol: Protocol, Renderer: id, RendererVersion: version, View: "chart", Artifact: artifact}
	job := Job{Protocol: Protocol, WorkDir: root.Name(), View: result.View, Theme: theme, Artifact: artifact,
		Renderer: Module{ID: id, Version: version}, Components: []Module{}, Catalog: items}
	return service.publish(ctx, root, result, job)
}
