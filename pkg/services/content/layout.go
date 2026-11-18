package content

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func ConfigFor(project string) (*model.MindConfig, error) {
	config, err := app.LoadProjectConfig(project, project)
	if err != nil {
		return nil, err
	}
	if config == nil {
		return nil, &output.Error{Kind: "not_found", Message: "mind.yaml missing"}
	}
	return config, nil
}

func reserved(config *model.MindConfig) map[string]bool {
	names := map[string]bool{}
	for _, name := range []string{"build", "outputs", "docs", "sources", "assets", "prompts", "thinking", "templates", "cache", "target", "node_modules", "vendor", "dist", "_archived"} {
		names[name] = true
	}
	if layout := config.Layout; layout != nil {
		for _, value := range []*string{layout.Articles, layout.Sources, layout.Assets, layout.Templates, layout.BuildOutput} {
			if value != nil {
				first, _, _ := strings.Cut(*value, "/")
				names[first] = true
			}
		}
	}
	for _, name := range config.ReservedRoots {
		names[name] = true
	}
	return names
}

func ValidateMindName(config *model.MindConfig, name string) error {
	if err := ValidateName(name); err != nil {
		return err
	}
	for reserved := range reserved(config) {
		if textcase.ASCIIEqualFold(reserved, name) {
			return output.Usage("reserved mind name: "+name, "")
		}
	}
	return nil
}

type child struct {
	name      string
	directory bool
}

func children(project, relative string, excluded map[string]bool) ([]child, error) {
	path, err := CheckedPath(project, relative)
	if err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(path)
	if os.IsNotExist(err) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var result []child
	for _, entry := range entries {
		name := strings.ToValidUTF8(entry.Name(), "\ufffd")
		if strings.HasPrefix(name, ".") || excluded[name] {
			continue
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return nil, output.Usage("symlink content refused: "+filepath.Join(path, entry.Name()), "")
		}
		result = append(result, child{name, entry.IsDir()})
	}
	sort.Slice(result, func(i, j int) bool { return result[i].name < result[j].name })
	return result, nil
}

func OutputRoot(layout, mind string) string {
	if layout == "bundle" {
		return mind + "/outputs"
	}
	return "outputs/" + mind
}

func markdownStem(name string) string {
	for strings.HasSuffix(name, ".md") {
		name = strings.TrimSuffix(name, ".md")
	}
	return name
}

func Discover(root, project string) ([]model.Mind, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	if config.ContentLayout == nil {
		return nil, output.Usage("project has no content_layout; use a new project with --layout category|bundle; legacy ownership requires explicit migration", "")
	}
	mode := *config.ContentLayout
	projectIdentity, err := filepath.Rel(root, project)
	if err != nil {
		return nil, err
	}
	if projectIdentity == "." {
		projectIdentity = ""
	}
	if projectIdentity == ".." || strings.HasPrefix(projectIdentity, ".."+string(filepath.Separator)) {
		return nil, output.Usage(fmt.Sprintf("path '%s' is not under project root '%s'", project, root), "")
	}
	projectIdentity = strings.ReplaceAll(filepath.ToSlash(projectIdentity), "\\", "/")
	if strings.Contains(projectIdentity, "::") {
		return nil, output.Usage("project identity cannot contain ::", "")
	}
	names := map[string]bool{}
	for name := range config.Minds {
		names[name] = true
	}
	excluded := reserved(config)
	childRoots, err := ChildProjectRoots(root, project)
	if err != nil {
		return nil, err
	}
	for name := range childRoots {
		excluded[name] = true
	}
	if mode == "bundle" {
		entries, err := children(project, "", excluded)
		if err != nil {
			return nil, err
		}
		for _, entry := range entries {
			if entry.directory {
				names[entry.name] = true
			}
		}
	} else {
		for _, role := range []string{"outputs", "prompts", "thinking", "sources", "assets"} {
			entries, err := children(project, role, nil)
			if err != nil {
				return nil, err
			}
			for _, entry := range entries {
				if entry.directory {
					names[entry.name] = true
				} else if (role == "prompts" || role == "thinking") && strings.HasSuffix(entry.name, ".md") {
					names[markdownStem(entry.name)] = true
				}
			}
		}
	}
	ordered := make([]string, 0, len(names))
	for name := range names {
		ordered = append(ordered, name)
	}
	sort.Strings(ordered)
	folded := map[string]bool{}
	result := make([]model.Mind, 0, len(ordered))
	for _, name := range ordered {
		if err := ValidateName(name); err != nil {
			return nil, err
		}
		collision := folded[textcase.Lower(name)]
		for reserved := range excluded {
			if textcase.ASCIIEqualFold(reserved, name) {
				collision = true
				break
			}
		}
		if collision {
			return nil, output.Usage("reserved or case-colliding mind name: "+name, "")
		}
		folded[textcase.Lower(name)] = true
		identity := projectIdentity + "::" + name
		registration := config.Minds[name]
		title := name
		if registration.Title != nil {
			title = *registration.Title
		}
		mind := model.Mind{Identity: identity, ProjectIdentity: projectIdentity, MindIdentity: identity, Name: name, Title: title, DefaultOutput: registration.DefaultOutput, Roles: model.JSONObject[string]{}, Outputs: model.JSONArray[model.Output]{}}
		outputRoot := OutputRoot(mode, name)
		entries, err := children(project, outputRoot, nil)
		if err != nil {
			return nil, err
		}
		outputs := map[string]model.Output{}
		for _, entry := range entries {
			if !entry.directory && !strings.HasSuffix(entry.name, ".md") {
				continue
			}
			outputName := entry.name
			if !entry.directory {
				outputName = markdownStem(outputName)
			}
			if err := ValidateName(outputName); err != nil {
				return nil, err
			}
			path := outputRoot + "/" + entry.name
			if entry.directory {
				if _, err := children(project, path, nil); err != nil {
					return nil, err
				}
			}
			key := textcase.Lower(outputName)
			if _, exists := outputs[key]; exists {
				hint := "remove the conflicting file or directory manually, or pick a different title"
				return nil, &output.Error{Kind: "shape_conflict", Message: fmt.Sprintf("cannot create unique output '%s': a same-name output with the same name already exists; remove it manually or pick a different title", filepath.Join(project, filepath.FromSlash(path))), Hint: &hint}
			}
			outputIdentity := identity + "::" + outputName
			shape := "file"
			if entry.directory {
				shape = "directory"
			}
			outputs[key] = model.Output{Identity: outputIdentity, ProjectIdentity: projectIdentity, MindIdentity: identity, OutputIdentity: outputIdentity, Mind: name, Name: outputName, Path: path, Shape: shape, BuildPath: "build/" + name + "/" + outputName + ".md"}
		}
		keys := make([]string, 0, len(outputs))
		for key := range outputs {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			mind.Outputs = append(mind.Outputs, outputs[key])
		}
		for _, role := range []struct{ kind, category, bundle string }{
			{"prompt", "prompts/" + name + ".md", name + "/prompt.md"}, {"thinking", "thinking/" + name + ".md", name + "/thinking.md"}, {"sources", "sources/" + name, name + "/sources"}, {"assets", "assets/" + name, name + "/assets"},
		} {
			path := role.category
			if mode == "bundle" {
				path = role.bundle
			}
			checked, err := CheckedPath(project, path)
			if err != nil {
				return nil, err
			}
			if _, err := os.Stat(checked); err == nil {
				mind.Roles[role.kind] = path
			} else if !os.IsNotExist(err) {
				return nil, err
			}
		}
		if mode == "bundle" {
			checked, err := CheckedPath(project, name)
			if err != nil {
				return nil, err
			}
			if info, err := os.Stat(checked); err == nil && info.IsDir() {
				mind.Path = &name
			}
		}
		result = append(result, mind)
	}
	return result, nil
}

func ProjectPaths(root string) ([]string, error) {
	manifest, err := store.ReadManifest(filepath.Join(root, "minds.yaml"))
	if err != nil {
		return nil, err
	}
	paths := map[string]bool{root: true}
	for _, entry := range manifest.Projects {
		if entry.ArchivedAt == nil {
			paths[filepath.Join(root, strings.TrimPrefix(entry.Path, "./"))] = true
		}
	}
	directory := strings.Trim(manifest.ProjectsDir, "/")
	entries, _ := os.ReadDir(filepath.Join(root, directory))
	for _, entry := range entries {
		path := filepath.Join(root, directory, entry.Name())
		if info, err := os.Stat(path); err == nil && info.IsDir() {
			paths[path] = true
		}
	}
	var result []string
	for path := range paths {
		if info, err := os.Stat(filepath.Join(path, "mind.yaml")); err == nil && !info.IsDir() {
			result = append(result, path)
		}
	}
	sort.Strings(result)
	return result, nil
}

func ChildProjectRoots(root, project string) (map[string]bool, error) {
	result := map[string]bool{}
	if _, err := os.Stat(filepath.Join(root, "minds.yaml")); os.IsNotExist(err) {
		return result, nil
	} else if err != nil {
		return nil, err
	}
	paths, err := ProjectPaths(root)
	if err != nil {
		return nil, err
	}
	for _, child := range paths {
		relative, err := filepath.Rel(project, child)
		if err != nil || relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			continue
		}
		first, _, _ := strings.Cut(filepath.ToSlash(relative), "/")
		result[first] = true
	}
	if filepath.Clean(root) == filepath.Clean(project) {
		manifest, err := store.ReadManifest(filepath.Join(root, "minds.yaml"))
		if err != nil {
			return nil, err
		}
		for _, part := range strings.Split(filepath.ToSlash(manifest.ProjectsDir), "/") {
			if part != "" && part != "." && part != ".." {
				result[part] = true
				break
			}
		}
	}
	return result, nil
}
