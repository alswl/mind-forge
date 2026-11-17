package content

import (
	"os"
	"path/filepath"
	"time"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"github.com/alswl/mind-forge/internal/textcase"
	"go.yaml.in/yaml/v3"
)

type MindCreateOptions struct {
	Title, DefaultOutput *string
	DryRun               bool
	Now                  time.Time
}

func creationTime(now time.Time) string {
	if now.IsZero() {
		now = time.Now()
	}
	return now.UTC().Format(time.RFC3339Nano)
}

func CreateMind(root, project, name string, options MindCreateOptions) (map[string]any, error) {
	if err := ValidateName(name); err != nil {
		return nil, err
	}
	if options.DefaultOutput != nil {
		if err := ValidateName(*options.DefaultOutput); err != nil {
			return nil, err
		}
	}
	minds, err := Discover(root, project)
	if err != nil {
		return nil, err
	}
	for _, mind := range minds {
		if textcase.ASCIIEqualFold(mind.Name, name) {
			return nil, fileExists(filepath.Join(project, name))
		}
	}
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	if err := ValidateMindName(config, name); err != nil {
		return nil, err
	}
	identity, err := ProjectIdentityPath(root, project)
	if err != nil {
		return nil, err
	}
	identity += "::" + name
	var proposed any
	if *config.ContentLayout == "bundle" {
		proposed = name
		if _, err := CheckedPath(project, name); err != nil {
			return nil, err
		}
	}
	if !options.DryRun {
		path := filepath.Join(project, "mind.yaml")
		raw, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		document, err := store.ParseYAML(raw)
		if err != nil {
			return nil, output.ParseError(path, err.Error())
		}
		var node *yaml.Node
		if len(document.Root.Content) > 0 {
			node = yamlField(document.Root.Content[0], "minds")
		}
		if node != nil {
			node, err = model.CloneYAMLValue(node)
			if err != nil {
				return nil, output.ParseError(path, err.Error())
			}
		}
		if node == nil || node.Kind != yaml.MappingNode {
			node = &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		}
		registration := &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		for _, field := range []struct {
			name  string
			value *string
		}{{"title", options.Title}, {"default_output", options.DefaultOutput}} {
			value := &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!null", Value: "null"}
			if field.value != nil {
				value = yamlString(*field.value)
			}
			setYAMLField(registration, field.name, value)
		}
		setYAMLField(node, name, registration)
		rendered, err := encodeYAML(map[string]*yaml.Node{"minds": node})
		if err != nil {
			return nil, err
		}
		// Registration does not materialize a bundle or any optional role.
		// The first authored output/role establishes an actual directory path.
		if err := (store.AtomicWriter{}).Write(path, document.ReplaceSection("minds", rendered), 0o644); err != nil {
			return nil, err
		}
	}
	return map[string]any{"kind": "mind", "identity": identity, "created_at": creationTime(options.Now), "dry_run": options.DryRun, "path": nil, "details": map[string]any{"proposed_path": proposed}}, nil
}
