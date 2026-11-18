package publish

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"go.yaml.in/yaml/v3"
)

type Publisher struct {
	View   model.PublisherView
	Target model.PublishTarget
}
type Discovery struct {
	Publishers  []Publisher
	Diagnostics []model.PublisherDiagnosticView
}
type ResolvedTarget struct {
	Target model.PublishTarget
	Base   string
}

func nameError(name string) (kind, message, hint string) {
	switch {
	case name == "":
		return "invalid_name", "publisher name cannot be empty", "provide a non-empty name in the definition file or use the filename stem"
	case name == "default" || name == "all" || name == "none":
		return "reserved_name", fmt.Sprintf("publisher name '%s' is reserved", name), "choose a different name; reserved names: default, all, none"
	}
	for _, c := range name {
		if c != '-' && (c < 'a' || c > 'z') && (c < '0' || c > '9') {
			return "invalid_name", fmt.Sprintf("publisher name '%s' is not valid kebab-case", name), "use only lowercase ASCII letters, digits, and hyphens"
		}
	}
	if strings.HasPrefix(name, "-") || strings.HasSuffix(name, "-") {
		return "invalid_name", fmt.Sprintf("publisher name '%s' cannot start or end with a hyphen", name), "remove leading or trailing hyphens from the name"
	}
	return "", "", ""
}

func secretKey(value any, prefix string) string {
	object, ok := value.(map[string]any)
	if !ok {
		return ""
	}
	keys := make([]string, 0, len(object))
	for key := range object {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		path := prefix + "." + key
		switch key {
		case "token", "password", "secret", "credential", "api_key", "access_key":
			return path
		}
		if found := secretKey(object[key], path); found != "" {
			return found
		}
	}
	return ""
}

func Discover(root string) (Discovery, error) {
	report := Discovery{Publishers: []Publisher{}, Diagnostics: []model.PublisherDiagnosticView{}}
	dir := filepath.Join(root, ".mind-forge/publisher")
	entries, err := os.ReadDir(dir)
	if err != nil {
		return report, nil
	}
	for _, entry := range entries {
		path := filepath.Join(dir, entry.Name())
		info, err := os.Stat(path)
		if err != nil || !info.Mode().IsRegular() {
			continue
		}
		ext := filepath.Ext(path)
		if ext != ".yaml" && ext != ".yml" {
			continue
		}
		relative := ".mind-forge/publisher/" + entry.Name()
		diag := func(kind string, name *string, message, hint string) {
			report.Diagnostics = append(report.Diagnostics, model.PublisherDiagnosticView{Kind: kind, Path: &relative, PublisherName: name, Message: message, Hint: &hint})
		}
		text, err := readText(path)
		if err != nil {
			diag("malformed_yaml", nil, "cannot read publisher file: "+err.Error(), "check file permissions for "+path)
			continue
		}
		document, err := store.ParseYAML([]byte(text))
		metadata := struct {
			Name, Label, Description, Type *string
			RequiredInputs                 []string `yaml:"required_inputs"`
		}{RequiredInputs: []string{}}
		if err == nil {
			err = document.DecodeMapping(&metadata)
		}
		if err != nil {
			diag("malformed_yaml", nil, "publisher definition is not valid YAML: "+err.Error(), "fix "+relative+" or remove the file")
			continue
		}
		name := strings.TrimSuffix(entry.Name(), ext)
		if metadata.Name != nil {
			name = *metadata.Name
		}
		var target model.PublishTarget
		if metadata.Type != nil {
			node := *document.Root.Content[0]
			node.Content = append([]*yaml.Node{}, node.Content...)
			if metadata.Name == nil {
				node.Content = append(node.Content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: "name"}, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: name})
			}
			if err = node.Decode(&target); err != nil {
				diag("malformed_yaml", nil, "publisher definition is not valid YAML: "+err.Error(), "fix "+relative+" or remove the file")
				continue
			}
		}
		kind, message, hint := nameError(name)
		invalid := kind != ""
		if invalid {
			diag(kind, &name, message, hint)
		}
		if metadata.Type == nil {
			diag("missing_required_field", &name, "publisher definition is missing required field: type", "add `type: local` or another valid type to "+relative)
			continue
		}
		if target.Type == "local" && configString(target, "path") == "" {
			diag("missing_required_field", &name, "local publisher definition is missing required field: config.path", "add `config.path: <directory>` to "+relative)
		}
		secret := secretKey(configMap(target), "config")
		if secret != "" {
			diag("secret_field", &name, "publisher definition contains a secret-bearing field: "+secret, "move secret values to environment variables or explicit CLI parameters and declare required_inputs instead")
		}
		duplicate := false
		for _, p := range report.Publishers {
			if p.View.Name == name {
				duplicate = true
				break
			}
		}
		if duplicate {
			diag("duplicate_name", &name, "duplicate publisher name in "+relative, "use a unique publisher name or remove the duplicate definition file")
		}
		if !invalid && !duplicate && secret == "" {
			target.Path = nil
			target.Prefix = nil
			target.BookSlug = nil
			target.Namespace = nil
			if prefix, ok := configMap(target)["prefix"].(string); ok {
				target.Prefix = &prefix
			}
			status := "available"
			if !target.Enabled {
				status = "disabled"
			}
			view := model.PublisherView{Name: name, Label: metadata.Label, Description: metadata.Description, TargetType: target.Type, Enabled: target.Enabled, SourcePath: relative, Status: status, RequiredInputs: metadata.RequiredInputs}
			report.Publishers = append(report.Publishers, Publisher{View: view, Target: target})
		}
	}
	sort.SliceStable(report.Publishers, func(i, j int) bool { return report.Publishers[i].View.Name < report.Publishers[j].View.Name })
	sort.SliceStable(report.Diagnostics, func(i, j int) bool {
		a, b := report.Diagnostics[i], report.Diagnostics[j]
		if *a.Path != *b.Path {
			return *a.Path < *b.Path
		}
		return a.Kind < b.Kind
	})
	return report, nil
}

func ResolveTarget(root, project string, config *model.MindConfig, requested *string) (ResolvedTarget, error) {
	name := requested
	if name == nil {
		name = config.Publish.DefaultTarget
	}
	if name == nil {
		return ResolvedTarget{}, output.Usage("no --target specified and no publish.default_target configured", "set publish.default_target in mind.yaml or pass --target <NAME>")
	}
	report, err := Discover(root)
	if err != nil {
		return ResolvedTarget{}, err
	}
	for _, publisher := range report.Publishers {
		if publisher.View.Name != *name {
			continue
		}
		for _, d := range report.Diagnostics {
			if d.PublisherName != nil && *d.PublisherName == *name && (d.Kind == "invalid_name" || d.Kind == "reserved_name" || d.Kind == "duplicate_name" || d.Kind == "secret_field") {
				return ResolvedTarget{}, output.Usage("publisher '"+*name+"' has configuration errors and cannot be used", "run `mf publish target list` for details")
			}
		}
		if !publisher.Target.Enabled {
			return ResolvedTarget{}, output.Usage("publisher '"+*name+"' is disabled", "set `enabled: true` in the publisher definition file")
		}
		return ResolvedTarget{Target: publisher.Target, Base: root}, nil
	}
	for _, d := range report.Diagnostics {
		if d.PublisherName != nil && *d.PublisherName == *name {
			return ResolvedTarget{}, output.Usage("publisher '"+*name+"' has configuration errors and cannot be used", "run `mf publish target list` for details")
		}
	}
	if config.Publish.Targets != nil {
		for _, target := range *config.Publish.Targets {
			if target.Name == *name {
				if !target.Enabled {
					return ResolvedTarget{}, output.Usage("publish target '"+*name+"' is disabled", "set `enabled: true` on the target in mind.yaml")
				}
				return ResolvedTarget{Target: target, Base: project}, nil
			}
		}
	}
	return ResolvedTarget{}, problem("not_found", "publish target '"+*name+"' not found in mind.yaml or .mind-forge/publisher/", "check the publisher name or `publish.targets[].name` in mind.yaml")
}
