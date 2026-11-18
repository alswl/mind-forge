package content

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func records(value any) ([]any, error) {
	raw, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var result []any
	if err := decoder.Decode(&result); err != nil {
		return nil, err
	}
	if result == nil {
		result = []any{}
	}
	return result, nil
}

func object(value any) map[string]any      { result, _ := value.(map[string]any); return result }
func stringValue(value any) (string, bool) { result, ok := value.(string); return result, ok }

func jsonEqual(left, right any) bool {
	a, errA := output.CompactJSON(left)
	b, errB := output.CompactJSON(right)
	return errA == nil && errB == nil && bytes.Equal(a, b)
}

func InspectContent(root, project string) (map[string]any, error) {
	minds, err := Discover(root, project)
	if err != nil {
		return nil, err
	}
	outputs := model.JSONArray[model.Output]{}
	for _, mind := range minds {
		outputs = append(outputs, mind.Outputs...)
	}
	additional, err := DiscoverAdditional(root, project)
	if err != nil {
		return nil, err
	}
	diskOutputs, err := records(outputs)
	if err != nil {
		return nil, err
	}
	diskAdditional, err := records(additional)
	if err != nil {
		return nil, err
	}
	index := map[string]any{}
	path := filepath.Join(project, "mind-index.yaml")
	if raw, err := os.ReadFile(path); err == nil {
		document, err := store.ParseYAML(raw)
		if err != nil {
			return nil, output.ParseError(path, err.Error())
		}
		if len(document.Root.Content) > 0 {
			value, err := model.JSONFromYAML(document.Root.Content[0])
			if err != nil {
				return nil, output.ParseError(path, err.Error())
			}
			index = object(value)
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	issues := []map[string]any{}
	for _, section := range []struct {
		name string
		disk []any
	}{{"outputs", diskOutputs}, {"additional", diskAdditional}} {
		expected := map[string]any{}
		for _, row := range section.disk {
			identity, _ := stringValue(object(row)["identity"])
			expected[identity] = row
		}
		var rows []any
		if value := index[section.name]; value != nil {
			var ok bool
			rows, ok = value.([]any)
			if !ok {
				issues = append(issues, map[string]any{"kind": "invalid_projection", "section": section.name, "candidates": section.disk})
				continue
			}
		}
		seen := map[string]bool{}
		for _, row := range rows {
			identity, _ := stringValue(object(row)["identity"])
			actual, exists := expected[identity]
			if seen[identity] {
				issues = append(issues, map[string]any{"kind": "duplicate_projection", "section": section.name, "identity": identity, "indexed": row, "candidates": actual})
			}
			seen[identity] = true
			if !exists {
				issues = append(issues, map[string]any{"kind": "stale_projection", "section": section.name, "identity": identity, "indexed": row, "candidates": section.disk})
			} else if !jsonEqual(actual, row) {
				issues = append(issues, map[string]any{"kind": "projection_mismatch", "section": section.name, "identity": identity, "indexed": row, "candidates": []any{actual}})
			}
		}
		identities := make([]string, 0, len(expected))
		for identity := range expected {
			identities = append(identities, identity)
		}
		sort.Strings(identities)
		for _, identity := range identities {
			if !seen[identity] {
				issues = append(issues, map[string]any{"kind": "missing_projection", "section": section.name, "identity": identity, "candidates": []any{expected[identity]}})
			}
		}
	}
	for _, mind := range minds {
		if mind.DefaultOutput == nil {
			continue
		}
		exists := false
		candidates := []string{}
		for _, item := range mind.Outputs {
			candidates = append(candidates, item.Identity)
			if item.Name == *mind.DefaultOutput {
				exists = true
			}
		}
		if !exists {
			issues = append(issues, map[string]any{"kind": "missing_default_output", "identity": mind.Identity, "default_output": *mind.DefaultOutput, "candidates": candidates})
		}
	}
	for _, section := range []string{"sources", "assets", "prompts", "thinking", "publish_records"} {
		type binding struct {
			key   string
			value any
		}
		rows := []binding{}
		switch value := index[section].(type) {
		case nil:
		case []any:
			for _, row := range value {
				rows = append(rows, binding{"", row})
			}
		case map[string]any:
			keys := make([]string, 0, len(value))
			for key := range value {
				keys = append(keys, key)
			}
			sort.Strings(keys)
			for _, key := range keys {
				rows = append(rows, binding{key, value[key]})
			}
		default:
			issues = append(issues, map[string]any{"kind": "invalid_binding", "section": section, "candidates": []any{}})
			continue
		}
		bindings := map[string][]string{}
		for _, binding := range rows {
			row := object(binding.value)
			path, hasPath := stringValue(row["path"])
			if !hasPath && strings.Contains(binding.key, "/") {
				path, hasPath = binding.key, true
			}
			if !hasPath {
				issues = append(issues, map[string]any{"kind": "binding_path_missing", "section": section, "key": binding.key, "candidates": []any{}})
				continue
			}
			relative := strings.TrimPrefix(path, "./")
			safe, err := CheckedPath(project, relative)
			if err != nil {
				issues = append(issues, map[string]any{"kind": "unsafe_binding", "section": section, "key": binding.key, "path": path, "candidates": []any{}})
				continue
			}
			if _, err := os.Stat(safe); err != nil {
				issues = append(issues, map[string]any{"kind": "missing_binding_target", "section": section, "key": binding.key, "path": path, "candidates": []any{}})
				continue
			}
			owners := []model.Mind{}
			role := section
			if role == "prompts" {
				role = "prompt"
			}
			for _, mind := range minds {
				owned := false
				if section == "publish_records" {
					for _, item := range mind.Outputs {
						if pathPrefix(relative, item.Path) {
							owned = true
							break
						}
					}
				} else if path, ok := mind.Roles[role]; ok && pathPrefix(relative, path) {
					owned = true
				}
				if owned {
					owners = append(owners, mind)
				}
			}
			candidates := []string{}
			for _, owner := range owners {
				candidates = append(candidates, owner.Identity)
			}
			claimed, present := stringValue(row["mind_identity"])
			if !present {
				claimed, present = stringValue(row["mind"])
			}
			if len(owners) != 1 || present && claimed != owners[0].Identity && claimed != owners[0].Name {
				var claim any
				if present {
					claim = claimed
				}
				issues = append(issues, map[string]any{"kind": "binding_mismatch", "section": section, "key": binding.key, "path": path, "claimed": claim, "candidates": candidates})
			}
			bindings[relative] = append(bindings[relative], binding.key)
		}
		paths := make([]string, 0, len(bindings))
		for path := range bindings {
			paths = append(paths, path)
		}
		sort.Strings(paths)
		for _, path := range paths {
			if len(bindings[path]) > 1 {
				issues = append(issues, map[string]any{"kind": "duplicate_binding", "section": section, "path": path, "candidates": bindings[path]})
			}
		}
	}
	keys := make(map[int]string, len(issues))
	for index, issue := range issues {
		raw, err := output.CompactJSON(issue)
		if err != nil {
			return nil, err
		}
		keys[index] = string(raw)
	}
	type orderedIssue struct {
		key   string
		value map[string]any
	}
	ordered := make([]orderedIssue, len(issues))
	for index, issue := range issues {
		ordered[index] = orderedIssue{keys[index], issue}
	}
	sort.SliceStable(ordered, func(i, j int) bool { return ordered[i].key < ordered[j].key })
	for index, issue := range ordered {
		issues[index] = issue.value
	}
	identity, err := ProjectIdentityPath(root, project)
	if err != nil {
		return nil, err
	}
	return map[string]any{"kind": "content_check", "project_identity": identity, "consistent": len(issues) == 0, "issues": issues, "disk": map[string]any{"outputs": diskOutputs, "additional": diskAdditional}}, nil
}

func pathPrefix(path, parent string) bool {
	path = filepath.Clean(path)
	parent = filepath.Clean(parent)
	return path == parent || strings.HasPrefix(path, parent+string(filepath.Separator))
}

func ReconcileContent(root, project string, dryRun bool) (map[string]any, error) {
	report, err := InspectContent(root, project)
	if err != nil {
		return nil, err
	}
	disk := report["disk"].(map[string]any)
	if !dryRun {
		path := filepath.Join(project, "mind-index.yaml")
		index, err := store.ReadIndex(path, false)
		if err != nil {
			return nil, err
		}
		updated := index.Value
		extra := make([]model.ExtraField, 0, len(updated.Extra)+2)
		for _, field := range updated.Extra {
			if field.Key != "outputs" && field.Key != "additional" {
				extra = append(extra, field)
			}
		}
		for _, section := range []string{"outputs", "additional"} {
			extra = append(extra, model.ExtraField{Key: section, Value: disk[section]})
		}
		updated.Extra = extra
		if err := index.Write(path, updated, false); err != nil {
			return nil, err
		}
	}
	return map[string]any{"kind": "content_index", "outputs": disk["outputs"], "additional": disk["additional"], "differences": report["issues"], "dry_run": dryRun}, nil
}
