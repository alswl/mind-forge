package asset

import (
	"os"
	"path/filepath"
	"sort"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type AddOptions struct {
	Name        *string
	Tags        []string
	Link, Force bool
}

func fileExists(name string) error {
	hint := "pass --force to overwrite"
	return &output.Error{Kind: "file_exists", Message: "refusing to overwrite existing file: " + name, Hint: &hint}
}

func (service Service) Add(project, cwd, input string, options AddOptions) (model.Asset, error) {
	canonical, err := filepath.EvalSymlinks(joinPath(cwd, input))
	if err != nil {
		return model.Asset{}, err
	}
	info, err := os.Stat(canonical)
	if err != nil {
		return model.Asset{}, err
	}
	if !info.Mode().IsRegular() {
		return model.Asset{}, output.Usage("source path '"+input+"' must be an existing regular file", "")
	}
	layout, err := projectLayout(project)
	if err != nil {
		return model.Asset{}, err
	}
	directory := joinPath(project, layout.Assets)
	if err := os.MkdirAll(directory, 0755); err != nil {
		return model.Asset{}, err
	}
	basename := assetFileName(input)
	if basename == "" {
		return model.Asset{}, output.Usage("cannot extract filename from '"+input+"'", "")
	}
	if _, err := store.CanonicalWithin(directory, canonical); err == nil {
		return model.Asset{}, output.Usage("source file is already inside the project's "+layout.Assets+"/ directory", "use `mf asset update <PATH>` to refresh metadata")
	}
	destination := filepath.Join(directory, basename)
	if _, err := os.Stat(destination); err == nil && !options.Force {
		path, err := filepath.EvalSymlinks(destination)
		if err != nil {
			path = destination
		}
		return model.Asset{}, fileExists(path)
	}
	size, hash, err := Metadata(canonical)
	if err != nil {
		return model.Asset{}, err
	}
	name := basename
	if options.Name != nil {
		name = *options.Name
	}
	item := model.Asset{Name: name, Kind: InferKind(destination), Path: layout.Assets + "/" + basename, Size: size, Hash: hash, Tags: append(model.JSONArray[string]{}, options.Tags...), AddedAt: service.timestamp()}
	document, err := readIndex(project)
	if err != nil {
		return model.Asset{}, err
	}
	items := assetEntries(document.Value)
	if options.Force {
		kept := []model.Asset{}
		for _, prior := range items {
			if prior.Path != item.Path {
				kept = append(kept, prior)
			}
		}
		items = kept
	}
	items = append(items, item)
	sort.SliceStable(items, func(i, j int) bool { return items[i].Name < items[j].Name })
	updated := document.Value
	updated.Assets = &items
	var raw []byte
	link := ""
	if options.Link {
		link = canonical
	} else {
		raw, err = os.ReadFile(canonical)
		if err != nil {
			return model.Asset{}, err
		}
	}
	mode := info.Mode() & (os.ModePerm | os.ModeSetuid | os.ModeSetgid | os.ModeSticky)
	err = store.PublishFile(destination, raw, mode, link, func() (bool, error) { return service.save(project, document, updated) })
	return item, err
}
