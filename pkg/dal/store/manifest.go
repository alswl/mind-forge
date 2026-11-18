package store

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"go.yaml.in/yaml/v3"
)

func ReadManifest(path string) (model.Manifest, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return model.Manifest{}, err
	}
	if len(bytes.TrimSpace(raw)) == 0 {
		return model.DefaultManifest(), nil
	}
	document, err := ParseYAML(raw)
	if err != nil {
		return model.Manifest{}, output.ParseError(path, err.Error())
	}
	var manifest model.Manifest
	if err := document.DecodeMapping(&manifest); err != nil {
		return model.Manifest{}, output.ParseError(path, err.Error())
	}
	if err := ValidateSchema(manifest.SchemaVersion, path); err != nil {
		return model.Manifest{}, err
	}
	for i := range manifest.Projects {
		entry := &manifest.Projects[i]
		if entry.CreatedAt != "" {
			continue
		}
		rawPath := entry.Path
		entry.Name = filepath.Base(strings.TrimPrefix(rawPath, "./"))
		if strings.Contains(rawPath, "/") || strings.HasPrefix(rawPath, ".") {
			entry.Path = "./" + strings.Trim(strings.TrimPrefix(rawPath, "./"), "/")
		} else {
			entry.Path = ProjectRelativePath(manifest.ProjectsDir, rawPath)
		}
	}
	return manifest, nil
}

func ProjectRelativePath(projectsDir, name string) string {
	directory := strings.Trim(projectsDir, "/")
	if directory == "" || directory == "." {
		return "./" + name
	}
	return "./" + directory + "/" + name
}

func WriteManifest(path string, manifest model.Manifest) error {
	root := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
	schema := &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: manifest.SchemaVersion, Style: yaml.SingleQuotedStyle}
	projects := &yaml.Node{Kind: yaml.SequenceNode, Tag: "!!seq"}
	for _, entry := range manifest.Projects {
		stripped := strings.TrimPrefix(entry.Path, "./")
		if strings.HasPrefix(entry.Path, "./") && !strings.Contains(stripped, "/") {
			stripped = entry.Path
		}
		projects.Content = append(projects.Content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: stripped})
	}
	root.Content = []*yaml.Node{{Kind: yaml.ScalarNode, Tag: "!!str", Value: "schema"}, schema,
		{Kind: yaml.ScalarNode, Tag: "!!str", Value: "projects_dir"}, {Kind: yaml.ScalarNode, Tag: "!!str", Value: manifest.ProjectsDir},
		{Kind: yaml.ScalarNode, Tag: "!!str", Value: "projects"}, projects}
	var raw bytes.Buffer
	encoder := yaml.NewEncoder(&raw)
	encoder.SetIndent(2)
	encoder.CompactSeqIndent()
	if err := encoder.Encode(root); err != nil {
		return err
	}
	if err := encoder.Close(); err != nil {
		return err
	}
	return (AtomicWriter{}).Write(path, raw.Bytes(), 0o644)
}
