package content

import (
	"bytes"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"go.yaml.in/yaml/v3"
)

func projectPrefix(project string) string {
	if project == "" {
		return ""
	}
	return project + "/"
}

func projectScoped(project, name string) string { return projectPrefix(project) + name }

func containingProject(name string, projects []string) (string, bool) {
	selected, found := "", false
	for _, project := range projects {
		if strings.HasPrefix(name, projectPrefix(project)) && (!found || len(project) > len(selected)) {
			selected, found = project, true
		}
	}
	return selected, found
}

func mappedIdentity(identity string, mappings map[string]*string) (string, bool) {
	matched := ""
	for old := range mappings {
		if (identity == old || strings.HasPrefix(identity, old+"::")) && len(old) > len(matched) {
			matched = old
		}
	}
	if matched == "" {
		return identity, true
	}
	if mappings[matched] == nil {
		return "", false
	}
	return *mappings[matched] + identity[len(matched):], true
}

func yamlMapping() *yaml.Node { return &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"} }

func snapshotMetadata(files lifecycle.Snapshot, name string) (*yaml.Node, error) {
	entry, exists := files[name]
	if !exists {
		return yamlMapping(), nil
	}
	if entry.Kind != "file" {
		return nil, lifecycleFailure("path_conflict", name)
	}
	doc, err := store.ParseYAML(entry.Bytes)
	if err != nil {
		return nil, lifecycleFailure("invalid_metadata", err.Error())
	}
	if len(doc.Root.Content) == 0 || doc.Root.Content[0].Tag == "!!null" {
		return yamlMapping(), nil
	}
	node, err := model.CloneYAMLValue(doc.Root.Content[0])
	if err != nil {
		return nil, lifecycleFailure("invalid_metadata", err.Error())
	}
	if node.Kind != yaml.MappingNode {
		return nil, lifecycleFailure("invalid_metadata", name+": expected mapping")
	}
	return node, nil
}

func yamlValuesEqual(left, right *yaml.Node) (bool, error) {
	if left == nil || right == nil {
		return left == right, nil
	}
	a, err := model.CloneYAMLValue(left)
	if err != nil {
		return false, err
	}
	b, err := model.CloneYAMLValue(right)
	if err != nil {
		return false, err
	}
	x, err := yaml.Marshal(a)
	if err != nil {
		return false, err
	}
	y, err := yaml.Marshal(b)
	return bytes.Equal(x, y), err
}

func (inventory *contentInventory) putMetadata(name string, value *yaml.Node) error {
	before, err := snapshotMetadata(inventory.before, name)
	if err != nil {
		return err
	}
	entry, exists := inventory.before[name]
	if !exists {
		entry = lifecycle.Entry{Kind: "file", Mode: 0o644}
	}
	raw, changed := entry.Bytes, false
	for i := 0; i < len(value.Content); i += 2 {
		key, section := value.Content[i].Value, value.Content[i+1]
		equal, err := yamlValuesEqual(yamlField(before, key), section)
		if err != nil {
			return err
		}
		if equal {
			continue
		}
		raw, err = spliceLifecycleSection(raw, key, section)
		if err != nil {
			return lifecycleFailure("invalid_metadata", err.Error())
		}
		changed = true
	}
	if changed {
		entry.Bytes = raw
		inventory.after[name] = entry
	}
	return nil
}

type authorityRow struct{ key, value *yaml.Node }

func authorityRows(value *yaml.Node) ([]authorityRow, error) {
	rows := []authorityRow{}
	if value == nil || value.Tag == "!!null" {
		return rows, nil
	}
	switch value.Kind {
	case yaml.MappingNode:
		for i := 0; i < len(value.Content); i += 2 {
			rows = append(rows, authorityRow{value.Content[i], value.Content[i+1]})
		}
	case yaml.SequenceNode:
		for _, row := range value.Content {
			rows = append(rows, authorityRow{value: row})
		}
	default:
		return nil, lifecycleFailure("invalid_metadata", "expected registration map or sequence")
	}
	return rows, nil
}

func nodeFromValue(value any) (*yaml.Node, error) {
	var node yaml.Node
	err := node.Encode(value)
	return &node, err
}

func (inventory *contentInventory) lifecycleMetadata(moves, identities map[string]*string, touched map[string]bool) error {
	paths, err := ProjectPaths(inventory.root)
	if err != nil {
		return err
	}
	projects := make([]string, 0, len(paths))
	indexes := map[string]*yaml.Node{}
	for _, path := range paths {
		project, err := ProjectIdentityPath(inventory.root, path)
		if err != nil {
			return err
		}
		projects = append(projects, project)
		indexes[project], err = snapshotMetadata(inventory.before, projectScoped(project, "mind-index.yaml"))
		if err != nil {
			return err
		}
	}
	sort.Strings(projects)
	for name, after := range inventory.after {
		before, exists := inventory.before[name]
		if exists && before.Kind == after.Kind && before.Mode == after.Mode && bytes.Equal(before.Bytes, after.Bytes) {
			continue
		}
		if project, found := containingProject(name, projects); found {
			touched[project] = true
		}
	}
	for _, section := range []string{"sources", "assets", "publish_records", "prompts", "thinking", "articles"} {
		byProject := map[string][]authorityRow{}
		for _, project := range projects {
			rows, err := authorityRows(yamlField(indexes[project], section))
			if err != nil {
				return lifecycleFailure("invalid_metadata", project+": "+section)
			}
			for _, row := range rows {
				field := "path"
				if section == "articles" {
					field = "article_path"
				}
				relative, present := yamlText(yamlField(row.value, field))
				if !present {
					if key, ok := yamlText(row.key); ok && strings.Contains(key, "/") {
						relative, present = key, true
					}
				}
				if !present {
					if touched[project] && (section == "sources" || section == "assets" || section == "articles") {
						return lifecycleFailure("ownership_unresolved", project+": "+section+" registration lacks explicit path")
					}
					byProject[project] = append(byProject[project], row)
					continue
				}
				trimmed := relative
				for strings.HasPrefix(trimmed, "./") {
					trimmed = trimmed[2:]
				}
				full := projectScoped(project, trimmed)
				if touched[project] && (section == "sources" || section == "assets") {
					unsafe := strings.HasPrefix(relative, "/")
					for _, part := range strings.Split(relative, "/") {
						unsafe = unsafe || part == ".."
					}
					if _, exists := inventory.before[full]; !exists || unsafe {
						return lifecycleFailure("ownership_unresolved", project+": "+section+" registration has an absent or external path: "+relative)
					}
				}
				mapped, exists := lifecycle.RelocatedPath(full, moves)
				if !exists {
					continue
				}
				target, found := containingProject(mapped, projects)
				if !found {
					return lifecycleFailure("invalid_metadata", "registration moved outside a known project")
				}
				if mapped != full {
					rel := strings.TrimPrefix(mapped, projectPrefix(target))
					if row.value.Kind != yaml.MappingNode {
						row.value = yamlMapping()
					}
					setYAMLField(row.value, field, yamlString(rel))
					if key, ok := yamlText(row.key); ok && key == relative {
						row.key = yamlString(rel)
					}
					if yamlField(row.value, "project") != nil {
						setYAMLField(row.value, "project", yamlString(target))
					}
				}
				byProject[target] = append(byProject[target], row)
			}
		}
		for _, project := range projects {
			rows := byProject[project]
			original := yamlField(indexes[project], section)
			if len(rows) == 0 && original == nil {
				continue
			}
			allKeys := true
			for _, row := range rows {
				allKeys = allKeys && row.key != nil
			}
			useMap := original != nil && original.Kind == yaml.MappingNode || (original == nil || original.Tag == "!!null") && allKeys
			value := &yaml.Node{Kind: yaml.SequenceNode, Tag: "!!seq"}
			if useMap {
				value = yamlMapping()
			}
			seen := map[string]bool{}
			for _, row := range rows {
				if useMap {
					key := row.key
					if key == nil {
						key = yamlField(row.value, "name")
					}
					if key == nil {
						return lifecycleFailure("invalid_metadata", "cannot determine registration key")
					}
					keyValue, err := model.JSONFromYAML(key)
					if err != nil {
						return err
					}
					encoded, err := output.CompactJSON(keyValue)
					if err != nil {
						return err
					}
					if seen[string(encoded)] {
						return lifecycleFailure("path_conflict", project+": duplicate "+section+" key")
					}
					seen[string(encoded)] = true
					value.Content = append(value.Content, key)
				}
				value.Content = append(value.Content, row.value)
			}
			setYAMLField(indexes[project], section, value)
		}
	}
	outputs := map[string]model.JSONArray[model.Output]{}
	additional := map[string][]Additional{}
	for _, project := range projects {
		absolute := filepath.Join(inventory.root, project)
		config, err := ConfigFor(absolute)
		if err != nil {
			return err
		}
		if config.ContentLayout == nil {
			continue
		}
		minds, err := Discover(inventory.root, absolute)
		if err != nil {
			return err
		}
		for _, mind := range minds {
			for _, item := range mind.Outputs {
				identity, keepIdentity := mappedIdentity(item.Identity, identities)
				name, keepPath := lifecycle.RelocatedPath(projectScoped(project, item.Path), moves)
				if !keepIdentity || !keepPath {
					continue
				}
				parts := strings.Split(identity, "::")
				if len(parts) != 3 || !strings.HasPrefix(name, projectPrefix(parts[0])) {
					return lifecycleFailure("invalid_metadata", identity)
				}
				item.Identity, item.OutputIdentity = identity, identity
				item.ProjectIdentity, item.Mind, item.Name = parts[0], parts[1], parts[2]
				item.MindIdentity = parts[0] + "::" + parts[1]
				item.Path = strings.TrimPrefix(name, projectPrefix(parts[0]))
				item.BuildPath = "build/" + parts[1] + "/" + parts[2] + ".md"
				outputs[parts[0]] = append(outputs[parts[0]], item)
			}
		}
		items, err := DiscoverAdditional(inventory.root, absolute)
		if err != nil {
			return err
		}
		for _, item := range items {
			owner, keepOwner := mappedIdentity(item.MindIdentity, identities)
			name, keepPath := lifecycle.RelocatedPath(projectScoped(project, item.Path), moves)
			if !keepOwner || !keepPath {
				continue
			}
			target, found := containingProject(name, projects)
			if !found || !touched[target] {
				continue
			}
			cfg, err := snapshotMetadata(inventory.after, projectScoped(target, "mind.yaml"))
			if err != nil {
				return err
			}
			mode, _ := yamlText(yamlField(cfg, "content_layout"))
			item.Path = strings.TrimPrefix(name, projectPrefix(target))
			parts := strings.Split(item.Path, "/")
			if len(parts) < 2 {
				return lifecycleFailure("invalid_metadata", name)
			}
			kind := parts[0]
			if mode == "bundle" {
				kind = strings.TrimSuffix(parts[1], ".md")
			}
			ownerParts := strings.Split(owner, "::")
			item.ProjectIdentity, item.MindIdentity, item.Mind = target, owner, ownerParts[len(ownerParts)-1]
			item.AdditionalKind, item.Identity = kind, owner+"::additional::"+kind
			additional[target] = append(additional[target], item)
		}
	}
	for _, project := range projects {
		if touched[project] {
			out := outputs[project]
			if out == nil {
				out = model.JSONArray[model.Output]{}
			}
			rows, err := records(out)
			if err != nil {
				return err
			}
			node, err := nodeFromValue(rows)
			if err != nil {
				return err
			}
			setYAMLField(indexes[project], "outputs", node)
			cfg, err := snapshotMetadata(inventory.after, projectScoped(project, "mind.yaml"))
			if err != nil {
				return err
			}
			if mode, explicit := yamlText(yamlField(cfg, "content_layout")); explicit && (mode == "category" || mode == "bundle") {
				items := additional[project]
				if items == nil {
					items = []Additional{}
				}
				sort.Slice(items, func(i, j int) bool { return items[i].Identity < items[j].Identity })
				rows, err := records(items)
				if err != nil {
					return err
				}
				node, err := nodeFromValue(rows)
				if err != nil {
					return err
				}
				setYAMLField(indexes[project], "additional", node)
			}
		}
		if err := inventory.putMetadata(projectScoped(project, "mind-index.yaml"), indexes[project]); err != nil {
			return err
		}
	}
	return nil
}
