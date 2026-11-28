package report

import (
	"bytes"
	_ "embed"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strings"

	dal "github.com/alswl/mind-forge/pkg/dal/report"
	"github.com/santhosh-tekuri/jsonschema/v6"
)

const Protocol = "report.mind-forge/v1"

type Kind string

const (
	Renderer  Kind = "Renderer"
	Component Kind = "Component"
)

var (
	ErrInvalid     = errors.New("invalid report configuration")
	ErrNotFound    = errors.New("report resource not found")
	ErrConflict    = errors.New("report already exists")
	ErrUnavailable = errors.New("report worker unavailable")

	slugPattern    = regexp.MustCompile(`^[a-z0-9]+(-[a-z0-9]+)*$`)
	versionPattern = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$`)
)

//go:embed assets/catalog.json
var builtins []byte

type manifest struct {
	Protocol    string   `yaml:"apiVersion"`
	Kind        Kind     `yaml:"kind"`
	ID          string   `yaml:"id"`
	Version     string   `yaml:"version"`
	Entry       string   `yaml:"entry"`
	DataSchema  string   `yaml:"dataSchema"`
	PropsSchema string   `yaml:"propsSchema"`
	Example     string   `yaml:"example"`
	Description string   `yaml:"description"`
	Views       []string `yaml:"views"`
	Components  []string `yaml:"components"`
}

type Item struct {
	Protocol    string          `json:"apiVersion"`
	Kind        Kind            `json:"kind"`
	ID          string          `json:"id"`
	Version     string          `json:"version"`
	Source      string          `json:"source"`
	Manifest    string          `json:"manifest,omitempty"`
	Entry       string          `json:"entry"`
	Schema      json.RawMessage `json:"schema"`
	Example     string          `json:"example,omitempty"`
	ExampleData json.RawMessage `json:"exampleData,omitempty"`
	Description string          `json:"description,omitempty"`
	Views       []string        `json:"views,omitempty"`
	Components  []string        `json:"components,omitempty"`
}

type Diagnostic struct {
	Kind    Kind   `json:"kind"`
	ID      string `json:"id,omitempty"`
	Path    string `json:"path"`
	Message string `json:"message"`
}

type Catalog struct {
	Items       []Item       `json:"items"`
	Diagnostics []Diagnostic `json:"diagnostics"`
}

type Service struct {
	Registry string
	Worker   Worker
}

func ValidSlug(slug string) bool { return len(slug) <= 100 && slugPattern.MatchString(slug) }

func validVersion(version string) bool {
	if !versionPattern.MatchString(version) {
		return false
	}
	core, _, _ := strings.Cut(version, "+")
	_, pre, _ := strings.Cut(core, "-")
	for id := range strings.SplitSeq(pre, ".") {
		if len(id) > 1 && id[0] == '0' && strings.Trim(id, "0123456789") == "" {
			return false
		}
	}
	return true
}

func validID(id string) bool {
	namespace, slug, ok := strings.Cut(id, "/")
	return ok && (namespace == "builtin" || namespace == "repo") && ValidSlug(slug)
}

func (service Service) Catalog(kind Kind, id string) (Catalog, error) {
	catalog, err := service.discover()
	if err != nil {
		return Catalog{}, err
	}
	if id != "" {
		if _, err := catalog.lookup(kind, id); err != nil {
			return Catalog{}, err
		}
	}
	catalog.Items = slices.DeleteFunc(catalog.Items, func(item Item) bool {
		return item.Kind != kind || id != "" && item.ID != id
	})
	catalog.Diagnostics = slices.DeleteFunc(catalog.Diagnostics, func(item Diagnostic) bool {
		return item.Kind != kind || id != "" && item.ID != id
	})
	return catalog, nil
}

func (catalog Catalog) lookup(kind Kind, id string) (Item, error) {
	if !validID(id) {
		return Item{}, fmt.Errorf("%w: invalid %s ID %q", ErrInvalid, kind, id)
	}
	for _, diagnostic := range catalog.Diagnostics {
		if diagnostic.Kind == kind && diagnostic.ID == id {
			return Item{}, fmt.Errorf("%w: %s: %s", ErrInvalid, diagnostic.Path, diagnostic.Message)
		}
	}
	for _, item := range catalog.Items {
		if item.Kind == kind && item.ID == id {
			return item, nil
		}
	}
	return Item{}, fmt.Errorf("%w: %s %q", ErrNotFound, kind, id)
}

func (service Service) discover() (Catalog, error) {
	catalog := Catalog{Diagnostics: []Diagnostic{}}
	if err := json.Unmarshal(builtins, &catalog.Items); err != nil {
		return Catalog{}, fmt.Errorf("embedded report catalog: %w", err)
	}
	if service.Registry != "" {
		root, err := os.OpenRoot(service.Registry)
		if err != nil {
			return Catalog{}, fmt.Errorf("report registry: %w", err)
		}
		defer root.Close()
		if err := dal.Regular(root, repoMarker); err != nil {
			return Catalog{}, fmt.Errorf("report registry: %w", err)
		}
		catalog.scan(root, Component, ".mind-forge/report/components")
		catalog.scan(root, Renderer, ".mind-forge/report/renderers")
	}
	for _, item := range catalog.Items {
		for _, dependency := range item.Components {
			if _, err := catalog.lookup(Component, dependency); err != nil {
				catalog.addDiagnostic(item.Kind, item.ID, item.Manifest, fmt.Errorf("component %q: %w", dependency, err))
			}
		}
	}
	slices.SortFunc(catalog.Items, func(a, b Item) int {
		return strings.Compare(string(a.Kind)+"/"+a.ID, string(b.Kind)+"/"+b.ID)
	})
	slices.SortFunc(catalog.Diagnostics, func(a, b Diagnostic) int {
		return strings.Compare(string(a.Kind)+"/"+a.ID+"/"+a.Path+"/"+a.Message, string(b.Kind)+"/"+b.ID+"/"+b.Path+"/"+b.Message)
	})
	return catalog, nil
}

func (catalog *Catalog) addDiagnostic(kind Kind, id, path string, err error) {
	catalog.Diagnostics = append(catalog.Diagnostics, Diagnostic{kind, id, path, err.Error()})
}

func (catalog *Catalog) scan(root *os.Root, kind Kind, directory string) {
	if err := dal.Check(root, directory, false); err != nil {
		if !errors.Is(err, fs.ErrNotExist) {
			catalog.addDiagnostic(kind, "", directory, err)
		}
		return
	}
	entries, err := fs.ReadDir(root.FS(), directory)
	if err != nil {
		catalog.addDiagnostic(kind, "", directory, err)
		return
	}
	claims := map[string]string{}
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".") || !entry.IsDir() && entry.Type()&fs.ModeSymlink == 0 {
			continue
		}
		id := "repo/" + entry.Name()
		dir := filepath.Join(directory, entry.Name())
		path := filepath.Join(dir, strings.ToLower(string(kind))+".yaml")
		var m manifest
		if err := dal.Manifest(root, path, &m); err != nil {
			catalog.addDiagnostic(kind, id, path, err)
			continue
		}
		if m.Kind == kind && strings.HasPrefix(m.ID, "repo/") && validID(m.ID) {
			if previous, exists := claims[m.ID]; exists {
				catalog.addDiagnostic(kind, m.ID, path, fmt.Errorf("duplicate ID %q also declared in %s", m.ID, previous))
			} else {
				claims[m.ID] = path
			}
		}
		item, err := readItem(root, dir, path, kind, id, m)
		if err != nil {
			catalog.addDiagnostic(kind, id, path, err)
			continue
		}
		catalog.Items = append(catalog.Items, item)
	}
}

func readItem(root *os.Root, dir, path string, kind Kind, id string, m manifest) (Item, error) {
	if m.Protocol != Protocol || m.Kind != kind || m.ID != id || !validID(id) || !validVersion(m.Version) {
		return Item{}, fmt.Errorf("expected apiVersion %s, kind %s, case-sensitive ID %q and semantic version", Protocol, kind, id)
	}
	schemaPath, err := m.validateKind()
	if err != nil {
		return Item{}, err
	}
	entry, err := itemFile(root, dir, "entry", m.Entry)
	if err != nil {
		return Item{}, err
	}
	schemaFile, err := itemFile(root, dir, "schema", schemaPath)
	if err != nil {
		return Item{}, err
	}
	raw, err := dal.Read(root, schemaFile)
	if err != nil {
		return Item{}, err
	}
	schema, err := compileSchema(raw)
	if err != nil {
		return Item{}, fmt.Errorf("%s: %w", schemaFile, err)
	}
	item := Item{Protocol: Protocol, Kind: kind, ID: id, Version: m.Version, Source: "repo", Manifest: path,
		Entry: entry, Schema: raw, Description: m.Description, Views: m.Views, Components: m.Components}
	example := m.Example
	if kind == Renderer && example == "" {
		if _, err := root.Lstat(filepath.Join(dir, "example.json")); !errors.Is(err, fs.ErrNotExist) {
			example = "example.json"
		}
	}
	if example != "" {
		item.Example, err = itemFile(root, dir, "example", example)
		if err != nil {
			return Item{}, err
		}
		if kind == Renderer {
			rawExample, err := dal.Read(root, item.Example)
			if err != nil {
				return Item{}, err
			}
			value, err := jsonschema.UnmarshalJSON(bytes.NewReader(rawExample))
			if err == nil {
				err = schema.Validate(value)
			}
			if err != nil {
				return Item{}, fmt.Errorf("%s: %w", item.Example, err)
			}
			item.ExampleData = rawExample
		}
	}
	return item, nil
}

func (m manifest) validateKind() (string, error) {
	if m.Kind == Component {
		if m.PropsSchema == "" || m.DataSchema != "" || m.Example == "" || len(m.Views)+len(m.Components) != 0 {
			return "", fmt.Errorf("component requires propsSchema and example and cannot declare renderer fields")
		}
		return m.PropsSchema, nil
	}
	if m.DataSchema == "" || m.PropsSchema != "" || len(m.Views) == 0 {
		return "", fmt.Errorf("renderer requires dataSchema and views and cannot declare propsSchema")
	}
	if err := unique(m.Views, validView); err != nil {
		return "", fmt.Errorf("views: %w", err)
	}
	if err := unique(m.Components, validID); err != nil {
		return "", fmt.Errorf("components: %w", err)
	}
	return m.DataSchema, nil
}

func unique(values []string, valid func(string) bool) error {
	seen := map[string]bool{}
	for _, value := range values {
		if !valid(value) || seen[value] {
			return fmt.Errorf("invalid or repeated value %q", value)
		}
		seen[value] = true
	}
	return nil
}

func itemFile(root *os.Root, dir, field, value string) (string, error) {
	if err := dal.Local(value); err != nil {
		return "", fmt.Errorf("%s: %w", field, err)
	}
	path := filepath.Join(dir, value)
	if err := dal.Regular(root, path); err != nil {
		return "", fmt.Errorf("%s: %w", field, err)
	}
	return path, nil
}

type offlineLoader struct{}

func (offlineLoader) Load(url string) (any, error) {
	return nil, fmt.Errorf("external schema reference %q is unsupported", url)
}

func compileSchema(raw []byte) (*jsonschema.Schema, error) {
	doc, err := jsonschema.UnmarshalJSON(bytes.NewReader(raw))
	if err != nil {
		return nil, err
	}
	compiler := jsonschema.NewCompiler()
	compiler.UseLoader(offlineLoader{})
	const location = "https://mind-forge.invalid/report.schema.json"
	if err := compiler.AddResource(location, doc); err != nil {
		return nil, err
	}
	return compiler.Compile(location)
}
