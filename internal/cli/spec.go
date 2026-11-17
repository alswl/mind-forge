package cli

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"strings"
)

//go:embed spec_data.json
var specData []byte

type Access string

const (
	Read     Access = "read"
	Write    Access = "write"
	Recovery Access = "recovery"
)

type FlagSpec struct {
	Name      string   `json:"name"`
	Short     string   `json:"short"`
	Kind      string   `json:"kind"`
	Bits      int      `json:"bits,omitempty"`
	Repeat    bool     `json:"repeat"`
	Required  bool     `json:"required"`
	Hidden    bool     `json:"hidden"`
	ValueName string   `json:"value_name"`
	Help      string   `json:"help"`
	Default   *string  `json:"default,omitempty"`
	Choices   []string `json:"choices,omitempty"`
	Aliases   []string `json:"aliases"`
	Conflicts []string `json:"conflicts"`
	Requires  []string `json:"requires"`
}

type ArgumentSpec struct {
	Name     string   `json:"name"`
	Kind     string   `json:"kind"`
	Required bool     `json:"required"`
	Repeat   bool     `json:"repeat"`
	Choices  []string `json:"choices,omitempty"`
}

type CommandSpec struct {
	Path               []string       `json:"path"`
	Aliases            []string       `json:"aliases"`
	About              string         `json:"about"`
	Usage              string         `json:"usage"`
	Help               string         `json:"help"`
	HelpDefault        string         `json:"help_default"`
	RequiredSubcommand bool           `json:"required_subcommand"`
	RepoRequired       bool           `json:"repo_required"`
	Access             Access         `json:"access"`
	Flags              []FlagSpec     `json:"flags"`
	Arguments          []ArgumentSpec `json:"arguments"`
}

type Spec struct {
	Baseline    string        `json:"baseline"`
	GlobalFlags []FlagSpec    `json:"global_flags"`
	Commands    []CommandSpec `json:"commands"`
	byPath      map[string]int
	children    map[string]map[string]string
}

func LoadSpec() (*Spec, error) {
	var spec Spec
	if err := json.Unmarshal(specData, &spec); err != nil {
		return nil, err
	}
	spec.byPath = map[string]int{}
	spec.children = map[string]map[string]string{}
	for index, command := range spec.Commands {
		key := strings.Join(command.Path, " ")
		if _, exists := spec.byPath[key]; exists {
			return nil, fmt.Errorf("duplicate command %q", key)
		}
		spec.byPath[key] = index
		if command.Access != Read && command.Access != Write && command.Access != Recovery {
			return nil, fmt.Errorf("unknown access for %q", key)
		}
		if len(command.Path) == 0 {
			continue
		}
		parent := strings.Join(command.Path[:len(command.Path)-1], " ")
		if spec.children[parent] == nil {
			spec.children[parent] = map[string]string{}
		}
		name := command.Path[len(command.Path)-1]
		for _, spelling := range append([]string{name}, command.Aliases...) {
			if _, exists := spec.children[parent][spelling]; exists {
				return nil, fmt.Errorf("duplicate command or alias %q under %q", spelling, parent)
			}
			spec.children[parent][spelling] = name
		}
	}
	spec.GlobalFlags = append(spec.GlobalFlags, FlagSpec{Name: "help", Short: "h", Kind: "bool", Help: "Print help"}, FlagSpec{Name: "version", Short: "V", Kind: "bool", Help: "Print version"})
	return &spec, nil
}

func (spec *Spec) Lookup(path []string) (CommandSpec, bool) {
	var canonical []string
	for _, component := range path {
		name, exists := spec.children[strings.Join(canonical, " ")][component]
		if !exists {
			return CommandSpec{}, false
		}
		canonical = append(canonical, name)
	}
	index, exists := spec.byPath[strings.Join(canonical, " ")]
	if !exists {
		return CommandSpec{}, false
	}
	return spec.Commands[index], true
}

func (spec *Spec) Flag(path []string, spelling string) (FlagSpec, bool) {
	command, exists := spec.Lookup(path)
	if !exists {
		return FlagSpec{}, false
	}
	flags := append(append([]FlagSpec{}, command.Flags...), spec.GlobalFlags...)
	for _, flag := range flags {
		if spelling == "--"+flag.Name || flag.Short != "" && spelling == "-"+flag.Short {
			return flag, true
		}
		for _, alias := range flag.Aliases {
			if spelling == "--"+alias {
				return flag, true
			}
		}
	}
	return FlagSpec{}, false
}

type Modifiers struct{ DryRun, Check, Fix bool }

func (spec *Spec) Classify(path []string, modifiers Modifiers) Access {
	command, known := spec.Lookup(path)
	if !known {
		return Write
	}
	if command.Access == Recovery {
		return Recovery
	}
	if modifiers.DryRun || modifiers.Check {
		return Read
	}
	if len(command.Path) > 0 && command.Path[len(command.Path)-1] == "lint" && modifiers.Fix {
		return Write
	}
	return command.Access
}

func (spec *Spec) RequiresRepo(path []string) bool {
	command, known := spec.Lookup(path)
	return !known || command.RepoRequired
}
