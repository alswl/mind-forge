package model

import (
	"fmt"

	"go.yaml.in/yaml/v3"
)

const SchemaVersion = "1"

type ProjectEntry struct {
	Name       string  `json:"name" yaml:"name"`
	Path       string  `json:"path" yaml:"path"`
	CreatedAt  string  `json:"created_at" yaml:"created_at"`
	ArchivedAt *string `json:"archived_at,omitempty" yaml:"archived_at,omitempty"`
}

func (entry *ProjectEntry) UnmarshalYAML(node *yaml.Node) error {
	if node.Kind == yaml.ScalarNode && node.Tag == "!!str" {
		*entry = ProjectEntry{Name: node.Value, Path: node.Value}
		return nil
	}
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := requireFields(fields, "name", "path", "created_at"); err != nil {
		return err
	}
	type wire ProjectEntry
	var result wire
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	*entry = ProjectEntry(result)
	return nil
}

type Manifest struct {
	SchemaVersion string                  `json:"schema_version" yaml:"schema_version"`
	ProjectsDir   string                  `json:"projects_dir" yaml:"projects_dir"`
	Projects      JSONArray[ProjectEntry] `json:"projects" yaml:"projects"`
}

func DefaultManifest() Manifest {
	return Manifest{SchemaVersion: SchemaVersion, ProjectsDir: "projects", Projects: []ProjectEntry{}}
}

func (manifest *Manifest) UnmarshalYAML(node *yaml.Node) error {
	fields, err := mapping(node)
	if err != nil {
		return err
	}
	if err := rejectNull(fields, "schema", "schema_version", "projects_dir", "projects"); err != nil {
		return err
	}
	if err := rejectNullItems(fields["projects"]); err != nil {
		return err
	}
	if fields["schema"] != nil && fields["schema_version"] != nil {
		return fmt.Errorf("duplicate field %q", "schema_version")
	}
	type wire Manifest
	result := wire(DefaultManifest())
	if err := decodeYAMLRecord(node, &result); err != nil {
		return err
	}
	if schema := fields["schema"]; schema != nil {
		if err := schema.Decode(&result.SchemaVersion); err != nil {
			return err
		}
	}
	*manifest = Manifest(result)
	return nil
}
