package content

import (
	"bytes"
	"context"
	"fmt"
	"path"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/managers/lifecycle"
	"go.yaml.in/yaml/v3"
)

func lifecycleFailure(kind, message string) error {
	return &output.Error{Kind: kind, Message: message, Lifecycle: true}
}

// PlanAdditional uses the repository inventory and shared transaction protocol;
// planning never creates a directory or writes an index.
func PlanAdditional(root, project, selector string, destination *string) (lifecycle.Plan, error) {
	if err := (lifecycle.Engine{}).CheckRecovery(root); err != nil {
		return lifecycle.Plan{}, err
	}
	items, err := DiscoverAdditional(root, project)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	item, err := SelectAdditional(items, selector)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	if destination != nil {
		if _, err := AdditionalRenameTarget(root, project, item, *destination, items); err != nil {
			return lifecycle.Plan{}, err
		}
	}
	inventory, err := readContentInventory(context.Background(), root)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	if _, err := inventory.ownedProject(project); err != nil {
		return lifecycle.Plan{}, err
	}
	source, err := inventory.projectPath(project, item.Path)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	var target *string
	operation := fmt.Sprintf("AdditionalRemove { identity: %q }", selector)
	if destination != nil {
		name, err := inventory.projectPath(project, *destination)
		if err != nil {
			return lifecycle.Plan{}, err
		}
		target = &name
		operation = fmt.Sprintf("AdditionalRename { identity: %q, destination: %q }", selector, *destination)
	}
	moves := map[string]*string{source: target}
	after, err := lifecycle.RelocateSnapshot(inventory.before, moves)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	inventory.after = after
	if err := inventory.rewriteAuthored(moves); err != nil {
		return lifecycle.Plan{}, err
	}
	projectID, err := ProjectIdentityPath(inventory.root, project)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	if err := inventory.lifecycleMetadata(moves, nil, map[string]bool{projectID: true}); err != nil {
		return lifecycle.Plan{}, err
	}
	if err := lifecycle.EnsureParents(inventory.after); err != nil {
		return lifecycle.Plan{}, err
	}
	return lifecycle.NewPlan(inventory.root, operation, []string{"."}, inventory.before, inventory.after)
}

func (inventory *contentInventory) ownedProject(project string) ([]lifecycle.OwnedRoot, error) {
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	minds, err := Discover(inventory.root, project)
	if err != nil {
		return nil, err
	}
	children, err := ChildProjectRoots(inventory.root, project)
	if err != nil {
		return nil, err
	}
	childNames := make([]string, 0, len(children))
	for name := range children {
		childNames = append(childNames, name)
	}
	sort.Strings(childNames)
	prefix, err := ProjectIdentityPath(inventory.root, project)
	if err != nil {
		return nil, err
	}
	if prefix != "" {
		prefix += "/"
	}
	files := lifecycle.Snapshot{}
	for name, entry := range inventory.before {
		if strings.HasPrefix(name, prefix) {
			files[strings.TrimPrefix(name, prefix)] = entry
		}
	}
	return lifecycle.CollectOwnedRoots(*config, minds, childNames, files)
}

func (inventory *contentInventory) rewriteAuthored(moves map[string]*string) error {
	names := make([]string, 0, len(inventory.before))
	for name := range inventory.before {
		names = append(names, name)
	}
	sort.Strings(names)
	errors := []error{}
	for _, name := range names {
		entry := inventory.before[name]
		ext := path.Ext(name)
		if entry.Kind != "file" || ext != ".md" && ext != ".html" && ext != ".htm" {
			continue
		}
		target, exists := lifecycle.RelocatedPath(name, moves)
		if !exists {
			continue
		}
		if !utf8.Valid(entry.Bytes) {
			return lifecycleFailure("reference_unsupported", "non UTF-8 authored file: "+name)
		}
		text, err := lifecycle.RewriteAuthoredReferences(string(entry.Bytes), name, target, moves)
		if err != nil {
			errors = append(errors, err)
			continue
		}
		entry.Bytes = []byte(text)
		inventory.after[target] = entry
	}
	if len(errors) > 0 {
		kind := "reference_unsupported"
		messages := make([]string, 0, len(errors))
		for _, err := range errors {
			if failure, ok := err.(*output.Error); ok && failure.Kind == "inbound_reference" {
				kind = "inbound_reference"
			}
			messages = append(messages, err.Error())
		}
		return lifecycleFailure(kind, strings.Join(messages, "\n"))
	}
	return nil
}

func yamlText(node *yaml.Node) (string, bool) {
	if node != nil && node.Kind == yaml.ScalarNode && node.Tag == "!!str" {
		return node.Value, true
	}
	return "", false
}

func spliceLifecycleSection(raw []byte, section string, value any) ([]byte, error) {
	doc, err := store.ParseYAML(raw)
	if err != nil {
		return nil, err
	}
	var root *yaml.Node
	if len(doc.Root.Content) > 0 {
		root = doc.Root.Content[0]
	}
	var replacement yaml.Node
	if err := replacement.Encode(value); err != nil {
		return nil, err
	}
	updates := map[string]*yaml.Node{section: &replacement}
	anchors := map[string]bool{}
	var collectAnchors func(*yaml.Node)
	collectAnchors = func(node *yaml.Node) {
		if node == nil {
			return
		}
		if node.Anchor != "" {
			anchors[node.Anchor] = true
		}
		for _, child := range node.Content {
			collectAnchors(child)
		}
	}
	collectAnchors(yamlField(root, section))
	var refers func(*yaml.Node) bool
	refers = func(node *yaml.Node) bool {
		if node.Kind == yaml.AliasNode && anchors[node.Value] {
			return true
		}
		for _, child := range node.Content {
			if refers(child) {
				return true
			}
		}
		return false
	}
	// Detach aliases to replaced anchors before removing their definitions. An
	// unrelated registration must keep its previous value rather than inherit
	// the edited path or become an invalid dangling alias.
	if root != nil && root.Kind == yaml.MappingNode {
		for changed := true; changed; {
			changed = false
			for i := 0; i < len(root.Content); i += 2 {
				name, node := root.Content[i].Value, root.Content[i+1]
				if updates[name] != nil || !refers(node) {
					continue
				}
				updates[name], err = model.CloneYAMLValue(node)
				if err != nil {
					return nil, err
				}
				collectAnchors(node)
				changed = true
			}
		}
	}
	crlf := bytes.Contains(raw, []byte("\r\n"))
	lineEndings := func(block []byte) []byte {
		if crlf {
			return bytes.ReplaceAll(block, []byte("\n"), []byte("\r\n"))
		}
		return block
	}
	// Flow/quoted top-level keys have no standalone text span. Preserve every
	// value through an expanded mapping when their syntax requires re-rendering.
	for name := range updates {
		if _, span := doc.Section(name); !span && yamlField(root, name) != nil {
			copy, err := model.CloneYAMLValue(root)
			if err != nil {
				return nil, err
			}
			for name, node := range updates {
				setYAMLField(copy, name, node)
			}
			block, err := yaml.Marshal(copy)
			return lineEndings(block), err
		}
	}
	for name, node := range updates {
		block, err := yaml.Marshal(map[string]any{name: node})
		if err != nil {
			return nil, err
		}
		block = lineEndings(block)
		if original, exists := doc.Section(name); exists {
			raw = bytes.Replace(raw, original, block, 1)
		} else {
			if len(raw) > 0 && raw[len(raw)-1] != '\n' {
				raw = append(raw, '\n')
			}
			raw = append(raw, block...)
		}
	}
	_, err = store.ParseYAML(raw)
	return raw, err
}
