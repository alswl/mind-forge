package content

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"go.yaml.in/yaml/v3"
)

type ProjectTarget struct{ RequestedPath, Path, ResolvedPath string }
type ProjectCreateOptions struct {
	Force, DryRun bool
	Layout        *string
	Now           time.Time
}

func ProjectIdentityPath(root, project string) (string, error) {
	relative, err := filepath.Rel(root, project)
	if err != nil {
		return "", err
	}
	if relative == "." {
		return "", nil
	}
	return strings.ReplaceAll(relative, "\\", "/"), nil
}

func ProjectTargetMissing(root, name string) (bool, error) {
	path, _, err := legacyProjectPath(root, name)
	if err != nil {
		return false, err
	}
	_, err = os.Stat(path)
	return os.IsNotExist(err), nil
}

func canonicalPath(path string) string {
	if absolute, err := filepath.Abs(path); err == nil {
		path = absolute
	}
	if resolved, err := filepath.EvalSymlinks(path); err == nil {
		path = resolved
	}
	return path
}

func loadManifest(root string) (model.Manifest, error) {
	manifest, err := store.ReadManifest(filepath.Join(root, "minds.yaml"))
	if os.IsNotExist(err) {
		return model.DefaultManifest(), nil
	}
	return manifest, err
}

func projectDir(root, container, name string) string {
	if filepath.IsAbs(name) {
		return name
	}
	container = strings.Trim(container, "/")
	return filepath.Join(root, container, name)
}

func NormalizeProjectTarget(root, cwd, input string) (ProjectTarget, error) {
	if input == "" {
		return ProjectTarget{}, output.Usage("project path cannot be empty", "provide a path under the repo root")
	}
	if err := store.RejectTraversal(input, "repo"); err != nil {
		return ProjectTarget{}, err
	}
	root, cwd = canonicalPath(root), canonicalPath(cwd)
	manifest, err := loadManifest(root)
	if err != nil {
		return ProjectTarget{}, err
	}
	var resolved string
	if input == "." || strings.ContainsAny(input, "/\\") {
		target := input
		if !filepath.IsAbs(target) {
			target = filepath.Join(cwd, target)
		}
		resolved, err = store.CanonicalWithin(root, target)
	} else if cwd != root {
		resolved, err = store.CanonicalWithin(root, filepath.Join(cwd, input))
		if err != nil {
			resolved, err = store.CanonicalWithin(root, projectDir(root, manifest.ProjectsDir, input))
		}
	} else {
		resolved, err = store.CanonicalWithin(root, projectDir(root, manifest.ProjectsDir, input))
	}
	if err != nil {
		return ProjectTarget{}, err
	}
	relative, err := filepath.Rel(root, resolved)
	if err != nil {
		return ProjectTarget{}, err
	}
	if relative == "." {
		relative = ""
	}
	return ProjectTarget{RequestedPath: input, Path: relative, ResolvedPath: resolved}, nil
}

func CreateProject(root, cwd, input string, options ProjectCreateOptions) (output.VerbResult, error) {
	target, err := NormalizeProjectTarget(root, cwd, input)
	if err != nil {
		return output.VerbResult{}, err
	}
	for parent := filepath.Dir(target.ResolvedPath); ; parent = filepath.Dir(parent) {
		if _, err := os.Stat(filepath.Join(parent, "mind.yaml")); err == nil {
			relative, _ := filepath.Rel(canonicalPath(root), parent)
			if relative == "." {
				relative = ""
			}
			return output.VerbResult{}, output.Usage(fmt.Sprintf("project path '%s' is inside another project '%s'", target.Path, relative), "create the project outside the existing project root")
		}
		if parent == canonicalPath(root) || filepath.Dir(parent) == parent {
			break
		}
	}
	if options.Layout != nil {
		if _, err := os.Stat(target.ResolvedPath); err == nil {
			return output.VerbResult{}, output.Usage("--layout is only for a new project; existing content requires explicit migration", "")
		}
	}
	result := output.VerbResult{Verb: output.Create, Kind: "project", Identity: target.Path, Path: &target.Path, DryRun: options.DryRun, Details: map[string]any{"requested_path": target.RequestedPath, "path": target.Path}}
	if options.DryRun {
		return result, nil
	}
	if !options.Force {
		if entries, err := os.ReadDir(target.ResolvedPath); err == nil && len(entries) > 0 {
			return result, fileExists(target.ResolvedPath)
		}
	}
	createdAt := options.Now
	if createdAt.IsZero() {
		createdAt = time.Now()
	}
	stamp := createdAt.UTC().Format(time.RFC3339)
	scaffolded := []string{}
	for _, directory := range []string{"docs", "sources", "assets"} {
		path := filepath.Join(target.ResolvedPath, directory)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			if err := os.MkdirAll(path, 0o777); err != nil {
				return result, err
			}
			scaffolded = append(scaffolded, directory)
		}
	}
	for _, name := range []string{"mind.yaml", "mind-index.yaml"} {
		path := filepath.Join(target.ResolvedPath, name)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			text := "schema: '1'\n"
			if name == "mind.yaml" && options.Layout != nil {
				text += "content_layout: " + *options.Layout + "\n"
			}
			if err := (store.AtomicWriter{}).Write(path, []byte(text), 0o644); err != nil {
				return result, err
			}
			scaffolded = append(scaffolded, name)
		}
	}
	entry, err := upsertProject(root, target.Path, stamp, options.Force)
	if err != nil {
		return result, err
	}
	result.Details["created_at"], result.Details["scaffolded"] = entry.CreatedAt, scaffolded
	return result, nil
}

func fileExists(path string) error {
	hint := "pass --force to overwrite"
	return &output.Error{Kind: "file_exists", Message: "refusing to overwrite existing file: " + path, Hint: &hint}
}

func upsertProject(root, path, stamp string, force bool) (model.ProjectEntry, error) {
	manifest, err := loadManifest(root)
	if err != nil {
		return model.ProjectEntry{}, err
	}
	for _, entry := range manifest.Projects {
		if entry.Name == path || entry.Path == path || entry.Path == "./"+path {
			if force {
				return entry, nil
			}
			return model.ProjectEntry{}, output.Usage(fmt.Sprintf("project path '%s' is already registered as '%s'", path, entry.Name), "project paths must be unique; use a different path")
		}
	}
	entry := model.ProjectEntry{Name: path, Path: "./" + path, CreatedAt: stamp}
	manifest.Projects = append(manifest.Projects, entry)
	return entry, store.WriteManifest(filepath.Join(root, "minds.yaml"), manifest)
}

func readProjectIndex(project string) model.Index {
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		return model.DefaultIndex()
	}
	return index.Value
}

func sliceLength[T any](values *[]T) uint64 {
	if values == nil {
		return 0
	}
	return uint64(len(*values))
}

func indexActivity(index model.Index) *string {
	var latest *string
	include := func(stamp string) {
		if latest == nil || stamp > *latest {
			copy := stamp
			latest = &copy
		}
	}
	if index.Articles != nil {
		for _, entry := range *index.Articles {
			include(entry.UpdatedAt)
		}
	}
	if index.Assets != nil {
		for _, entry := range *index.Assets {
			include(entry.AddedAt)
		}
	}
	if index.Sources != nil {
		for _, entry := range *index.Sources {
			include(entry.UpdatedAt)
		}
	}
	return latest
}

func ListProjects(root string) ([]model.ProjectListEntry, []string, error) {
	manifest, err := loadManifest(root)
	if err != nil {
		return nil, nil, err
	}
	result := make([]model.ProjectListEntry, 0, len(manifest.Projects))
	warnings := []string{}
	for _, entry := range manifest.Projects {
		path := entry.Path
		if !filepath.IsAbs(path) {
			path = filepath.Join(root, path)
		}
		index := model.DefaultIndex()
		document, err := store.ReadIndex(filepath.Join(path, "mind-index.yaml"), false)
		if err != nil {
			warnings = append(warnings, err.Error())
		} else {
			index = document.Value
			warnings = append(warnings, document.Warnings...)
		}
		stamp := entry.CreatedAt
		if stamp == "" {
			stamp = projectCreatedAt(path)
		}
		result = append(result, model.ProjectListEntry{Name: entry.Name, Path: entry.Path, CreatedAt: stamp, ArchivedAt: entry.ArchivedAt, DocumentCount: sliceLength(index.Articles) + sliceLength(index.Sources) + sliceLength(index.Assets) + sliceLength(index.Terms), LastActivityAt: indexActivity(index)})
	}
	sort.SliceStable(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, warnings, nil
}

func yamlField(node *yaml.Node, name string) *yaml.Node {
	if node == nil || node.Kind != yaml.MappingNode {
		return nil
	}
	for index := 0; index < len(node.Content); index += 2 {
		if node.Content[index].Value == name {
			return node.Content[index+1]
		}
	}
	return nil
}

func yamlString(value string) *yaml.Node {
	return &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: value}
}

func setYAMLField(node *yaml.Node, name string, value *yaml.Node) {
	for index := 0; index < len(node.Content); index += 2 {
		if node.Content[index].Value == name {
			if value == nil {
				node.Content = append(node.Content[:index], node.Content[index+2:]...)
			} else {
				node.Content[index+1] = value
			}
			return
		}
	}
	if value != nil {
		node.Content = append(node.Content, yamlString(name), value)
	}
}

func encodeYAML(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := yaml.NewEncoder(&buffer)
	encoder.SetIndent(2)
	encoder.CompactSeqIndent()
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	if err := encoder.Close(); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func UpdateProject(root, name string, description *string, clear, dryRun bool) (output.VerbResult, error) {
	result := output.VerbResult{Verb: output.Update, Kind: "project", Identity: name, DryRun: dryRun}
	if description == nil && !clear {
		return result, output.Usage("nothing to update: use --description or --clear-description", "pass --description <TEXT> or --clear-description")
	}
	if description != nil && clear {
		return result, output.Usage("--description and --clear-description are mutually exclusive", "")
	}
	project, _, err := legacyProjectPath(root, name)
	if err != nil {
		return result, err
	}
	path := filepath.Join(project, "mind.yaml")
	raw, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return result, output.Usage(fmt.Sprintf("project '%s' not found in Mind Repo", name), "use `mf project list` to see available projects")
	}
	if err != nil {
		return result, err
	}
	document, err := store.ParseYAML(raw)
	if err != nil {
		return result, output.ParseError(path, err.Error())
	}
	node := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
	if len(document.Root.Content) > 0 && document.Root.Content[0].Kind == yaml.MappingNode {
		node = document.Root.Content[0]
	}
	projectNode := yamlField(node, "project")
	if projectNode != nil {
		projectNode, err = model.CloneYAMLValue(projectNode)
		if err != nil {
			return result, output.ParseError(path, err.Error())
		}
	}
	var old *string
	if value := yamlField(projectNode, "description"); value != nil && value.Tag == "!!str" {
		text := value.Value
		old = &text
	} else if value := yamlField(node, "description"); value != nil && value.Tag == "!!str" {
		text := value.Value
		old = &text
	}
	relative, _ := filepath.Rel(root, project)
	result.Path = &relative
	result.Details = map[string]any{"changes": map[string]any{"description": map[string]any{"from": old, "to": description}}}
	if dryRun {
		return result, nil
	}
	if projectNode == nil || projectNode.Kind != yaml.MappingNode {
		projectNode = &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
	}
	if yamlField(projectNode, "name") == nil {
		setYAMLField(projectNode, "name", yamlString(name))
	}
	var value *yaml.Node
	if description != nil {
		value = yamlString(*description)
	}
	setYAMLField(projectNode, "description", value)
	rendered, err := encodeYAML(map[string]*yaml.Node{"project": projectNode})
	if err != nil {
		return result, err
	}
	updated := document.ReplaceSection("project", rendered)
	if node.Kind != yaml.MappingNode || len(document.Root.Content) == 0 || document.Root.Content[0].Kind != yaml.MappingNode {
		updated = rendered
	}
	if yamlField(node, "schema_version") == nil && yamlField(node, "schema") == nil {
		next, err := store.ParseYAML(updated)
		if err != nil {
			return result, err
		}
		updated = next.ReplaceSection("schema_version", []byte("schema_version: '1'\n"))
	}
	return result, (store.AtomicWriter{}).Write(path, updated, 0o644)
}

func ProjectDetails(project, name string) (model.ProjectDetails, error) {
	index := readProjectIndex(project)
	config, err := ConfigFor(project)
	if err != nil {
		return model.ProjectDetails{}, err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return model.ProjectDetails{}, err
	}
	details := model.ProjectDetails{Name: name, Path: "./" + name, ArticleCount: sliceLength(index.Articles), SourceCount: sliceLength(index.Sources), AssetCount: sliceLength(index.Assets), LastActive: indexActivity(index), Layout: &layout}
	if raw, err := os.ReadFile(filepath.Join(project, "mind.yaml")); err == nil {
		summary := &model.MindYamlSummary{SchemaVersion: "1", Types: model.JSONArray[string]{}, SourceDirs: model.JSONArray[string]{}, AssetsDir: "assets"}
		if document, err := store.ParseYAML(raw); err == nil && len(document.Root.Content) > 0 {
			root := document.Root.Content[0]
			if value := yamlField(root, "schema_version"); value != nil && value.Tag == "!!str" {
				summary.SchemaVersion = value.Value
			}
			if value := yamlField(root, "assets_dir"); value != nil && value.Tag == "!!str" {
				summary.AssetsDir = value.Value
			}
			for key, target := range map[string]*model.JSONArray[string]{"types": &summary.Types, "source_dirs": &summary.SourceDirs} {
				if value := yamlField(root, key); value != nil && value.Kind == yaml.SequenceNode {
					for _, item := range value.Content {
						if item.Tag == "!!str" {
							*target = append(*target, item.Value)
						}
					}
				}
			}
		}
		details.MindYamlSummary = summary
	}
	return details, nil
}
