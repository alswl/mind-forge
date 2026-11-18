package source

import (
	"context"
	"os"
	"path/filepath"
	"sort"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type AddOptions struct {
	Name, Kind, SourceKind            *string
	Link, Force, RegisterOnly, DryRun bool
}

type AddOutcome struct {
	Source           model.Source
	Mode             string
	Replaced         bool
	ReplacedLocation *string
}

func (service Service) Add(ctx context.Context, root, project, cwd, input string, options AddOptions) (AddOutcome, error) {
	if options.RegisterOnly {
		if options.Link {
			return AddOutcome{}, output.Usage("--register-only cannot be combined with --link", "")
		}
		if ClassifyInput(input) == URLInput {
			return AddOutcome{}, output.Usage("--register-only requires a local file path", "")
		}
	}
	if ClassifyInput(input) == URLInput {
		return service.addURL(ctx, project, input, options)
	}
	layout, err := projectLayout(project)
	if err != nil {
		return AddOutcome{}, err
	}
	sourcesDir := filepath.Join(project, layout.Sources)
	sourcePath, err := ResolveInput(input, project, sourcesDir, cwd, root)
	if err != nil {
		return AddOutcome{}, err
	}
	canonical, err := filepath.EvalSymlinks(sourcePath)
	if err != nil {
		return AddOutcome{}, err
	}
	info, err := os.Stat(canonical)
	if err != nil {
		return AddOutcome{}, err
	}
	if !info.Mode().IsRegular() {
		return AddOutcome{}, output.Usage("source path '"+input+"' must be an existing regular file", "")
	}
	_, insideError := store.CanonicalWithin(sourcesDir, canonical)
	if options.RegisterOnly && insideError != nil {
		return AddOutcome{}, output.Usage("--register-only path must be inside the project's "+layout.Sources+"/ directory", "")
	}
	if !options.RegisterOnly && insideError == nil {
		return AddOutcome{}, output.Usage("source file is already inside the project's "+layout.Sources+"/ directory", "pass --register-only to add the existing file to the source index")
	}
	kind := InferFileKind(canonical)
	if options.Kind != nil && *options.Kind != "auto" {
		kind = *options.Kind
	}
	if kind == "rss" || kind == "web" {
		if options.RegisterOnly {
			return AddOutcome{}, output.Usage("cannot use --file-kind rss or --file-kind web with a local file", "")
		}
		return AddOutcome{}, output.Usage("cannot use --type rss or --type web with a local file input", "pass an http(s):// URL")
	}
	name, err := DeriveNameFromPath(sourcePath)
	if options.Name != nil {
		name, err = *options.Name, nil
	}
	if err != nil {
		return AddOutcome{}, err
	}
	destination := filepath.Join(sourcesDir, kind, filepath.Base(sourcePath))
	relative, err := relativePath(project, destination)
	if err != nil {
		return AddOutcome{}, err
	}
	if options.RegisterOnly {
		relative, err = relativePath(project, canonical)
		if err != nil {
			return AddOutcome{}, err
		}
	}
	document, err := readIndex(project)
	if err != nil {
		return AddOutcome{}, err
	}
	items := sourceEntries(document.Value)
	for _, item := range items {
		if item.Path == nil || *item.Path != relative {
			continue
		}
		if item.Name != name {
			if options.RegisterOnly {
				return AddOutcome{}, output.Usage("source path '"+relative+"' is already registered as '"+item.Name+"'", "use the existing source name or update it explicitly")
			}
			return AddOutcome{}, output.Usage("destination '"+relative+"' already holds source '"+item.Name+"'", "choose a unique --name (or remove/rename the existing source first)")
		}
		if options.RegisterOnly && !options.Force {
			return AddOutcome{Source: item, Mode: "register"}, nil
		}
	}
	var suggestion *string
	if options.Name == nil {
		value := SuggestUniqueName(sourcePath, sourcesDir)
		suggestion = &value
	}
	result, updated, err := service.prepareAdd(document.Value, name, kind, options.SourceKind, nil, &relative, options.Force, suggestion)
	if err != nil {
		return AddOutcome{}, err
	}
	if options.RegisterOnly {
		result.Mode = "register"
		if !options.DryRun {
			_, err = service.save(project, document, updated)
		}
		return result, err
	}
	result.Mode = "copy"
	if options.Link {
		result.Mode = "link"
	}
	if options.DryRun {
		return result, nil
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0755); err != nil {
		return AddOutcome{}, err
	}
	var raw []byte
	if !options.Link {
		raw, err = os.ReadFile(canonical)
		if err != nil {
			return AddOutcome{}, err
		}
	}
	link := ""
	if options.Link {
		link = canonical
	}
	mode := info.Mode() & (os.ModePerm | os.ModeSetuid | os.ModeSetgid | os.ModeSticky)
	if err := store.PublishFile(destination, raw, mode, link, func() (bool, error) { return service.save(project, document, updated) }); err != nil {
		return AddOutcome{}, err
	}
	if result.ReplacedLocation != nil && *result.ReplacedLocation != relative {
		_ = os.Remove(joinPath(project, *result.ReplacedLocation))
	}
	return result, nil
}

func (service Service) prepareAdd(index model.Index, name, kind string, sourceKind, url, path *string, force bool, suggestion *string) (AddOutcome, model.Index, error) {
	items := sourceEntries(index)
	pos := -1
	for i, item := range items {
		if item.Name == name {
			pos = i
			break
		}
	}
	if pos >= 0 && !force {
		return AddOutcome{}, index, NameCollisionError(name, suggestion)
	}
	now := service.timestamp()
	entry := model.Source{Name: name, Kind: kind, SourceKind: sourceKind, URL: url, Path: path, Tags: model.JSONArray[string]{}, AddedAt: now, UpdatedAt: now}
	result := AddOutcome{Replaced: pos >= 0}
	if pos >= 0 {
		prior := items[pos]
		entry.Tags, entry.AddedAt, entry.Extra = prior.Tags, prior.AddedAt, prior.Extra
		result.ReplacedLocation = prior.Path
		if result.ReplacedLocation == nil && url != nil {
			result.ReplacedLocation = prior.URL
		}
		items = append(items[:pos], items[pos+1:]...)
		items = append(items, entry)
	} else {
		items = append(items, entry)
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].Name < items[j].Name })
	result.Source = entry
	index.Sources = &items
	return result, index, nil
}

func (service Service) addURL(ctx context.Context, project, input string, options AddOptions) (AddOutcome, error) {
	if err := ValidateURL(input); err != nil {
		return AddOutcome{}, err
	}
	if options.Name == nil {
		return AddOutcome{}, output.Usage("URL sources require an explicit --name", "pass --name <STRING>")
	}
	kind := "web"
	if options.Kind != nil && *options.Kind != "auto" {
		kind = *options.Kind
	}
	if kind == "pdf" || kind == "file" {
		return AddOutcome{}, output.Usage("cannot use --type "+kind+" with a URL input", "download the file first, then add the local path")
	}
	relative := filepath.ToSlash(filepath.Join("sources", kind, *options.Name+"."+FileExtensionForURL(input, kind)))
	document, err := readIndex(project)
	if err != nil {
		return AddOutcome{}, err
	}
	result, updated, err := service.prepareAdd(document.Value, *options.Name, kind, options.SourceKind, &input, &relative, options.Force, nil)
	if err != nil {
		return AddOutcome{}, err
	}
	result.Mode = "url"
	if options.DryRun {
		return result, nil
	}
	acquire := service.Acquire
	if acquire == nil {
		acquire = AcquireHTTP
	}
	raw, err := acquire(ctx, input, DefaultAcquisitionOptions())
	if err != nil {
		return AddOutcome{}, err
	}
	destination := joinPath(project, relative)
	if err := os.MkdirAll(filepath.Dir(destination), 0755); err != nil {
		return AddOutcome{}, err
	}
	if err := store.PublishFile(destination, raw, 0644, "", func() (bool, error) { return service.save(project, document, updated) }); err != nil {
		return AddOutcome{}, err
	}
	if result.ReplacedLocation != nil && *result.ReplacedLocation != relative {
		_ = os.Remove(joinPath(project, *result.ReplacedLocation))
	}
	return result, nil
}

func joinPath(project, relative string) string {
	if filepath.IsAbs(relative) {
		return relative
	}
	return filepath.Join(project, relative)
}
