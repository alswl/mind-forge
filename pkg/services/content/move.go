package content

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/managers/lifecycle"
	"go.yaml.in/yaml/v3"
)

type LifecycleOperation struct {
	Context                                     context.Context
	Kind, Identity, Name, Project, Mind, Layout string
}

func (operation LifecycleOperation) label() string {
	switch operation.Kind {
	case "convert":
		return "Convert(" + strings.ToUpper(operation.Layout[:1]) + operation.Layout[1:] + ")"
	case "mind-rename":
		return fmt.Sprintf("MindRename { identity: %q, name: %q }", operation.Identity, operation.Name)
	case "mind-move":
		return fmt.Sprintf("MindMove { identity: %q, project: %q }", operation.Identity, operation.Project)
	case "mind-remove":
		return fmt.Sprintf("MindRemove { identity: %q }", operation.Identity)
	case "output-rename":
		return fmt.Sprintf("OutputRename { identity: %q, name: %q }", operation.Identity, operation.Name)
	case "output-move":
		return fmt.Sprintf("OutputMove { identity: %q, mind: %q, project: %q }", operation.Identity, operation.Mind, operation.Project)
	default:
		return fmt.Sprintf("OutputRemove { identity: %q }", operation.Identity)
	}
}

type contentPlanner struct {
	inventory         *contentInventory
	project, mode     string
	config            *model.MindConfig
	owned             []lifecycle.OwnedRoot
	configs           map[string]*yaml.Node
	moves, identities map[string]*string
	touched, cleanup  map[string]bool
}

func mappingField(node *yaml.Node, key string) *yaml.Node {
	value := yamlField(node, key)
	if value == nil || value.Kind != yaml.MappingNode {
		value = yamlMapping()
		setYAMLField(node, key, value)
	}
	return value
}

func (planner *contentPlanner) configNode(project string) (*yaml.Node, error) {
	if value := planner.configs[project]; value != nil {
		return value, nil
	}
	value, err := snapshotMetadata(planner.inventory.before, projectScoped(project, "mind.yaml"))
	if err != nil {
		return nil, err
	}
	planner.configs[project] = value
	return value, nil
}

func mindRegistration(config *yaml.Node, owner model.Mind) (*yaml.Node, error) {
	item := yamlField(yamlField(config, "minds"), owner.Name)
	if item == nil || item.Kind != yaml.MappingNode {
		item = yamlMapping()
	} else {
		var err error
		item, err = model.CloneYAMLValue(item)
		if err != nil {
			return nil, err
		}
	}
	setYAMLField(item, "title", yamlString(owner.Title))
	return item, nil
}

func hiddenPlaceholder(files lifecycle.Snapshot, name string) bool {
	if entry, exists := files[name]; !exists || entry.Kind != "directory" {
		return false
	}
	for path := range files {
		if relative, found := strings.CutPrefix(path, name+"/"); found {
			first, _, _ := strings.Cut(relative, "/")
			if !strings.HasPrefix(first, ".") {
				return false
			}
		}
	}
	return true
}

func PlanLifecycle(root, project string, operation LifecycleOperation) (lifecycle.Plan, error) {
	task.Notify(operation.Context, task.Stage{Operation: "lifecycle", Name: "inventorying content"})
	if err := (lifecycle.Engine{}).CheckRecovery(root); err != nil {
		return lifecycle.Plan{}, err
	}
	config, err := ConfigFor(project)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	if config.ContentLayout == nil {
		return lifecycle.Plan{}, lifecycleFailure("ownership_unresolved", "explicit layout required")
	}
	inventory, err := readContentInventory(task.Context(operation.Context), root)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	owned, err := inventory.ownedProject(project)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	id, err := ProjectIdentityPath(inventory.root, project)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	planner := contentPlanner{inventory: inventory, project: id, mode: *config.ContentLayout, config: config, owned: owned,
		configs: map[string]*yaml.Node{}, moves: map[string]*string{}, identities: map[string]*string{}, touched: map[string]bool{id: true}, cleanup: map[string]bool{}}
	if _, err := planner.configNode(id); err != nil {
		return lifecycle.Plan{}, err
	}
	switch operation.Kind {
	case "convert":
		err = planner.convert(operation.Layout)
	case "mind-rename", "mind-move", "mind-remove":
		err = planner.mind(operation)
	case "output-rename", "output-move", "output-remove":
		err = planner.output(operation)
	default:
		err = lifecycleFailure("invalid_plan", "unknown lifecycle operation: "+operation.Kind)
	}
	if err != nil {
		return lifecycle.Plan{}, err
	}
	after, err := lifecycle.RelocateSnapshot(inventory.before, planner.moves)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	// Container creation performed by planning is separate from the moved roots.
	for name, entry := range inventory.after {
		if _, present := inventory.before[name]; !present {
			if existing, conflict := after[name]; conflict && existing.Kind != entry.Kind {
				return lifecycle.Plan{}, lifecycleFailure("path_conflict", name)
			}
			after[name] = entry
		}
	}
	inventory.after = after
	for name := range planner.cleanup {
		nonempty := false
		for path := range after {
			if strings.HasPrefix(path, name+"/") {
				nonempty = true
				break
			}
		}
		if !nonempty {
			delete(after, name)
		}
	}
	configNames := make([]string, 0, len(planner.configs))
	for name := range planner.configs {
		configNames = append(configNames, name)
	}
	sort.Strings(configNames)
	for _, name := range configNames {
		if err := inventory.putMetadata(projectScoped(name, "mind.yaml"), planner.configs[name]); err != nil {
			return lifecycle.Plan{}, err
		}
	}
	if err := inventory.rewriteAuthored(planner.moves); err != nil {
		return lifecycle.Plan{}, err
	}
	if err := inventory.lifecycleMetadata(planner.moves, planner.identities, planner.touched); err != nil {
		return lifecycle.Plan{}, err
	}
	if err := lifecycle.EnsureParents(after); err != nil {
		return lifecycle.Plan{}, err
	}
	return lifecycle.NewPlan(inventory.root, operation.label(), []string{"."}, inventory.before, inventory.after)
}

func (planner *contentPlanner) convert(target string) error {
	if target != "category" && target != "bundle" {
		return lifecycleFailure("invalid_metadata", "unknown content layout: "+target)
	}
	if target == planner.mode {
		return lifecycleFailure("layout_unchanged", "project already has requested layout")
	}
	minds, err := Discover(planner.inventory.root, filepath.Join(planner.inventory.root, planner.project))
	if err != nil {
		return err
	}
	names := map[string]string{}
	for _, owner := range minds {
		names[owner.Name] = owner.Name
	}
	for old, new := range lifecycle.OwnedMapping(planner.owned, target, names) {
		name := projectScoped(planner.project, new)
		planner.moves[projectScoped(planner.project, old)] = &name
	}
	cfg := planner.configs[planner.project]
	for _, owner := range minds {
		item, err := mindRegistration(cfg, owner)
		if err != nil {
			return err
		}
		setYAMLField(mappingField(cfg, "minds"), owner.Name, item)
		container := projectScoped(planner.project, owner.Name)
		if planner.mode == "bundle" {
			planner.cleanup[container] = true
		}
		if target == "bundle" {
			if _, exists := planner.inventory.before[container]; exists && !hiddenPlaceholder(planner.inventory.before, container) {
				return lifecycleFailure("path_conflict", container)
			}
			if _, exists := planner.inventory.after[container]; !exists {
				planner.inventory.after[container] = lifecycle.Entry{Kind: "directory", Mode: 0o755}
			}
		}
	}
	reserved := []string{}
	if target == "bundle" {
		roots := map[string]bool{}
		for _, name := range planner.config.ReservedRoots {
			roots[name] = true
		}
		for _, item := range planner.owned {
			first, _, _ := strings.Cut(item.Path, "/")
			roots[first] = true
		}
		for name := range roots {
			reserved = append(reserved, name)
		}
		sort.Strings(reserved)
	} else {
		roles := map[string]bool{}
		for _, item := range planner.owned {
			first, _, _ := strings.Cut(lifecycle.OwnedDestination(target, item.Mind, item.Kind, item.File), "/")
			roles[first] = true
		}
		for _, name := range planner.config.ReservedRoots {
			if !roles[name] {
				reserved = append(reserved, name)
			}
		}
	}
	value, err := nodeFromValue(reserved)
	if err != nil {
		return err
	}
	setYAMLField(cfg, "reserved_roots", value)
	setYAMLField(cfg, "content_layout", yamlString(target))
	return nil
}

func (planner *contentPlanner) mind(operation LifecycleOperation) error {
	root := planner.inventory.root
	project := filepath.Join(root, planner.project)
	owner, err := ResolveMind(root, project, operation.Identity)
	if err != nil {
		return err
	}
	destination, name, removing := planner.project, owner.Name, operation.Kind == "mind-remove"
	if operation.Kind == "mind-rename" {
		name = operation.Name
	} else if operation.Kind == "mind-move" {
		target, err := app.ResolveProject(root, &operation.Project, root, false)
		if err != nil {
			return err
		}
		destination, err = ProjectIdentityPath(root, target)
		if err != nil {
			return err
		}
	}
	mode := planner.mode
	if !removing {
		if destination == planner.project && name == owner.Name {
			return lifecycleFailure("path_conflict", "source and target are identical")
		}
		target := filepath.Join(root, destination)
		cfg, err := ConfigFor(target)
		if err != nil {
			return err
		}
		if err := ValidateMindName(cfg, name); err != nil {
			return err
		}
		children, err := ChildProjectRoots(root, target)
		if err != nil {
			return err
		}
		if children[name] {
			return lifecycleFailure("path_conflict", "mind name conflicts with child project container")
		}
		minds, err := Discover(root, target)
		if err != nil {
			return err
		}
		for _, mind := range minds {
			if textcase.ASCIIEqualFold(mind.Name, name) {
				return lifecycleFailure("path_conflict", "mind already exists: "+destination+"::"+name)
			}
		}
		if cfg.ContentLayout == nil {
			return lifecycleFailure("ownership_unresolved", "destination has no explicit layout")
		}
		mode = *cfg.ContentLayout
		planner.touched[destination] = true
		cfgNode, err := planner.configNode(destination)
		if err != nil {
			return err
		}
		item, err := mindRegistration(planner.configs[planner.project], owner)
		if err != nil {
			return err
		}
		setYAMLField(mappingField(cfgNode, "minds"), name, item)
	}
	if registrations := yamlField(planner.configs[planner.project], "minds"); registrations != nil && registrations.Kind == yaml.MappingNode {
		setYAMLField(registrations, owner.Name, nil)
	}
	var identity *string
	if !removing {
		value := destination + "::" + name
		identity = &value
	}
	planner.identities[owner.Identity] = identity
	if planner.mode == "bundle" && (mode == planner.mode || removing) {
		old := projectScoped(planner.project, owner.Name)
		if _, exists := planner.inventory.before[old]; exists {
			var target *string
			if !removing {
				value := projectScoped(destination, name)
				target = &value
			}
			planner.moves[old] = target
		}
	} else {
		for _, item := range planner.owned {
			if item.Mind != owner.Name {
				continue
			}
			var target *string
			if !removing {
				value := projectScoped(destination, lifecycle.OwnedDestination(mode, name, item.Kind, item.File))
				target = &value
			}
			planner.moves[projectScoped(planner.project, item.Path)] = target
		}
		if planner.mode == "bundle" {
			container := projectScoped(planner.project, owner.Name)
			for path := range planner.inventory.before {
				if !strings.HasPrefix(path, container+"/") {
					continue
				}
				covered := false
				for moved := range planner.moves {
					covered = covered || path == moved || strings.HasPrefix(path, moved+"/")
				}
				if !covered {
					return lifecycleFailure("ownership_unresolved", "bundle contains hidden container files without a category role; move cannot leave owned files behind")
				}
			}
			planner.cleanup[container] = true
		}
	}
	if !removing && mode == "bundle" && planner.mode != "bundle" {
		container := projectScoped(destination, name)
		if _, exists := planner.inventory.before[container]; exists {
			return lifecycleFailure("path_conflict", container)
		}
		planner.inventory.after[container] = lifecycle.Entry{Kind: "directory", Mode: 0o755}
	}
	return nil
}

func (planner *contentPlanner) output(operation LifecycleOperation) error {
	root := planner.inventory.root
	item, err := ResolveOutput(root, filepath.Join(root, planner.project), operation.Identity)
	if err != nil {
		return err
	}
	destination, owner, name, mode := planner.project, item.Mind, item.Name, planner.mode
	removing := operation.Kind == "output-remove"
	if operation.Kind == "output-rename" {
		if err := ValidateName(operation.Name); err != nil {
			return err
		}
		name = operation.Name
	} else if operation.Kind == "output-move" {
		target, err := app.ResolveProject(root, &operation.Project, root, false)
		if err != nil {
			return err
		}
		destination, err = ProjectIdentityPath(root, target)
		if err != nil {
			return err
		}
		mind, err := ResolveMind(root, target, operation.Mind)
		if err != nil {
			return err
		}
		owner = mind.Name
		cfg, err := ConfigFor(target)
		if err != nil {
			return err
		}
		if cfg.ContentLayout == nil {
			return lifecycleFailure("ownership_unresolved", "destination has no explicit layout")
		}
		mode = *cfg.ContentLayout
		planner.touched[destination] = true
	}
	var target, identity *string
	if !removing {
		mind, err := ResolveMind(root, filepath.Join(root, destination), owner)
		if err != nil {
			return err
		}
		for _, output := range mind.Outputs {
			if textcase.ASCIIEqualFold(output.Name, name) {
				return lifecycleFailure("path_conflict", "output exists: "+destination+"::"+owner+"::"+name)
			}
		}
		value := lifecycle.OwnedDestination(mode, owner, "outputs", false) + "/" + name
		if item.Shape == "file" {
			value += ".md"
		}
		value = projectScoped(destination, value)
		target = &value
		id := destination + "::" + owner + "::" + name
		identity = &id
	}
	planner.moves[projectScoped(planner.project, item.Path)] = target
	planner.identities[item.Identity] = identity
	registration := yamlField(yamlField(planner.configs[planner.project], "minds"), item.Mind)
	if selected, _ := yamlText(yamlField(registration, "default_output")); selected == item.Name {
		value := &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!null", Value: "null"}
		if !removing && destination == planner.project && owner == item.Mind {
			value = yamlString(name)
		}
		setYAMLField(registration, "default_output", value)
	}
	return nil
}
