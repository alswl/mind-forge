package app

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func fallback[T any](overlay, base *T) *T {
	if overlay != nil {
		return overlay
	}
	return base
}

func MergeConfig(base, overlay model.MindConfig) model.MindConfig {
	result := overlay
	result.Project = fallback(overlay.Project, base.Project)
	result.Publish.DefaultTarget = fallback(overlay.Publish.DefaultTarget, base.Publish.DefaultTarget)
	result.Publish.Targets = fallback(overlay.Publish.Targets, base.Publish.Targets)
	result.Publish.StripFrontMatter = fallback(overlay.Publish.StripFrontMatter, base.Publish.StripFrontMatter)
	result.Name = fallback(overlay.Name, base.Name)
	result.Description = fallback(overlay.Description, base.Description)
	result.Created = fallback(overlay.Created, base.Created)
	result.Updated = fallback(overlay.Updated, base.Updated)
	if result.Articles == nil {
		result.Articles = base.Articles
	}
	result.Templates = fallback(overlay.Templates, base.Templates)
	result.Plugins = fallback(overlay.Plugins, base.Plugins)
	return result
}

// LoadProjectConfig searches ancestors only when a repository boundary is known.
// Reading a configuration never materializes defaults on disk.
func LoadProjectConfig(cwd, repoRoot string) (*model.MindConfig, error) {
	current := canonical(cwd)
	boundary := ""
	if repoRoot != "" {
		boundary = canonical(repoRoot)
	}
	for {
		path := filepath.Join(current, "mind.yaml")
		if exists(path) {
			return readProjectConfig(path)
		}
		parent := filepath.Dir(current)
		if boundary == "" || current == boundary || parent == current {
			return nil, nil
		}
		current = parent
	}
}

func readProjectConfig(path string) (*model.MindConfig, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	if len(strings.TrimSpace(string(raw))) == 0 {
		config := model.DefaultMindConfig()
		return &config, nil
	}
	document, err := store.ParseYAML(raw)
	if err != nil {
		return nil, output.ParseError(path, err.Error())
	}
	var config model.MindConfig
	if err := document.DecodeMapping(&config); err != nil {
		return nil, output.ParseError(path, err.Error())
	}
	if config.SchemaVersion == "" {
		config.SchemaVersion = model.SchemaVersion
	}
	if err := store.ValidateSchema(config.SchemaVersion, path); err != nil {
		return nil, err
	}
	if config.Publish.Targets != nil {
		for i := range *config.Publish.Targets {
			target := &(*config.Publish.Targets)[i]
			if target.Config != nil {
				continue
			}
			values := map[string]any{}
			for name, value := range map[string]*string{"path": target.Path, "prefix": target.Prefix, "book_slug": target.BookSlug, "namespace": target.Namespace} {
				if value != nil {
					values[name] = *value
				}
			}
			if len(values) != 0 {
				target.Config = values
			}
		}
	}
	if config.Templates != nil {
		names := make([]string, 0, len(*config.Templates))
		for name := range *config.Templates {
			names = append(names, name)
		}
		sort.Strings(names)
		for _, name := range names {
			valid := len(name) > 0 && name[0] >= 'a' && name[0] <= 'z'
			for _, char := range name {
				valid = valid && (char >= 'a' && char <= 'z' || char >= '0' && char <= '9' || char == '_')
			}
			if !valid {
				hint := "rename template key to match ^[a-z][a-z0-9_]*$"
				return nil, &output.Error{Kind: "invalid_template_name", Message: fmt.Sprintf("invalid template name '%s'", name), Hint: &hint}
			}
		}
	}
	name := filepath.Base(filepath.Dir(path))
	if config.Name != nil {
		name = *config.Name
	}
	if config.Project == nil {
		config.Project = &model.ProjectMeta{Name: name, Description: config.Description, CreatedAt: config.Created}
	} else {
		if config.Project.Name == "" {
			config.Project.Name = name
		}
		if config.Project.Description == nil {
			config.Project.Description = config.Description
		}
	}
	return &config, nil
}

func ResolveEffectiveLayout(config *model.MindConfig) (model.EffectiveLayout, string) {
	layout := config.Layout
	if layout == nil {
		layout = &model.LayoutConfig{}
	}
	var conflicts []string
	resolve := func(name string, canonical *string, compatName, compat, defaultValue string) string {
		if canonical == nil {
			return compat
		}
		if *canonical != compat && compat != defaultValue {
			conflicts = append(conflicts, fmt.Sprintf("%s=\"%s\" overrides %s=\"%s\"", name, *canonical, compatName, compat))
		}
		return *canonical
	}
	result := model.EffectiveLayout{}
	result.Articles = resolve("layout.articles", layout.Articles, "paths.docs", config.Paths.Docs, "docs")
	result.Sources = resolve("layout.sources", layout.Sources, "paths.sources", config.Paths.Sources, "sources")
	result.Assets = resolve("layout.assets", layout.Assets, "paths.assets", config.Paths.Assets, "assets")
	result.Templates = "templates"
	if layout.Templates != nil {
		result.Templates = *layout.Templates
	}
	result.BuildOutput = resolve("layout.build_output", layout.BuildOutput, "build.output_dir", config.Build.OutputDir, "outputs")
	return result, strings.Join(conflicts, "; ")
}

type layoutEntry struct{ name, value string }

func layoutEntries(layout model.EffectiveLayout) []layoutEntry {
	return []layoutEntry{{"layout.articles", layout.Articles}, {"layout.sources", layout.Sources}, {"layout.assets", layout.Assets}, {"layout.templates", layout.Templates}, {"layout.build_output", layout.BuildOutput}}
}

func normalizeLayoutPath(value string) (string, bool) {
	var parts []string
	for _, part := range strings.Split(strings.TrimSpace(value), string(filepath.Separator)) {
		switch part {
		case "", ".":
		case "..":
			if len(parts) == 0 {
				return "", false
			}
			parts = parts[:len(parts)-1]
		default:
			parts = append(parts, part)
		}
	}
	return filepath.Join(parts...), len(parts) != 0
}

func resolveLayoutPath(path string) (string, error) {
	probe := path
	var suffix []string
	for {
		resolved, err := filepath.EvalSymlinks(probe)
		if err == nil {
			return filepath.Join(append([]string{resolved}, suffix...)...), nil
		}
		if info, statErr := os.Lstat(probe); statErr == nil && info.Mode()&os.ModeSymlink != 0 {
			return "", err
		}
		if !errors.Is(err, os.ErrNotExist) {
			return "", err
		}
		parent := filepath.Dir(probe)
		if parent == probe {
			return "", err
		}
		suffix = append([]string{filepath.Base(probe)}, suffix...)
		probe = parent
	}
}

func ValidateLayout(projectRoot string, layout model.EffectiveLayout) error {
	entries := layoutEntries(layout)
	for _, entry := range entries {
		if strings.TrimSpace(entry.value) == "" {
			return output.Usage(entry.name+" is empty or whitespace-only", "provide a project-relative directory name, e.g. "+entry.name+": my_dir")
		}
	}
	for _, entry := range entries {
		if filepath.IsAbs(entry.value) {
			return output.Usage(fmt.Sprintf("%s is an absolute path: '%s'", entry.name, entry.value), "use a project-relative directory name without a leading /")
		}
	}
	for _, entry := range entries {
		normalized, valid := normalizeLayoutPath(entry.value)
		escape := func() error {
			return output.Usage(fmt.Sprintf("%s escapes the project root: '%s'", entry.name, entry.value), "use a directory name that stays inside the project")
		}
		if !valid {
			return escape()
		}
		root := canonical(projectRoot)
		fullPath := filepath.Join(root, normalized)
		resolved, err := resolveLayoutPath(fullPath)
		if err != nil {
			return escape()
		}
		relative, err := filepath.Rel(root, resolved)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return escape()
		}
		if info, err := os.Stat(fullPath); err == nil && info.Mode().IsRegular() {
			return output.Usage(fmt.Sprintf("%s points to an existing file: '%s'", entry.name, entry.value), "remove the file or choose a different directory name")
		}
	}
	seen := map[string]string{}
	for _, entry := range entries {
		normalized, _ := normalizeLayoutPath(entry.value)
		if previous, ok := seen[normalized]; ok {
			return output.Usage(fmt.Sprintf("%s and %s resolve to the same path: '%s'", entry.name, previous, entry.value), "each layout category must use a unique directory name")
		}
		seen[normalized] = entry.name
	}
	return nil
}

func ResolveAndValidateLayout(projectRoot string, config *model.MindConfig) (model.EffectiveLayout, error) {
	layout, _ := ResolveEffectiveLayout(config)
	config.Layout = &model.LayoutConfig{Articles: &layout.Articles, Sources: &layout.Sources, Assets: &layout.Assets, Templates: &layout.Templates, BuildOutput: &layout.BuildOutput}
	return layout, ValidateLayout(projectRoot, layout)
}
