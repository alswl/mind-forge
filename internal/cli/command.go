package cli

import (
	"errors"
	"fmt"
	"io"
	"os"
	"slices"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/version"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

type Handler func(*cobra.Command, CommandSpec, []string) error

func NewCommand(handler Handler) (*cobra.Command, error) {
	spec, err := LoadSpec()
	if err != nil {
		return nil, err
	}
	commands := make(map[string]*cobra.Command, len(spec.Commands))
	for _, item := range spec.Commands {
		name := "mf"
		if len(item.Path) != 0 {
			name = item.Path[len(item.Path)-1]
		}
		command := &cobra.Command{
			Use: name, Short: item.About, Aliases: item.Aliases,
			Version:       version.Current().Version,
			SilenceErrors: true, SilenceUsage: true, DisableSuggestions: true,
			Args: validateCommand,
			RunE: func(command *cobra.Command, args []string) error {
				if handler != nil {
					return handler(command, item, args)
				}
				return nil
			},
		}
		command.SetVersionTemplate("mf {{.Version}}\n")
		command.SetFlagErrorFunc(flagParseError)
		command.ValidArgsFunction = func(current *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
			position := len(args)
			if position >= len(item.Arguments) && len(item.Arguments) > 0 && item.Arguments[len(item.Arguments)-1].Repeat {
				position = len(item.Arguments) - 1
			}
			if position < len(item.Arguments) && len(item.Arguments[position].Choices) != 0 {
				return item.Arguments[position].Choices, cobra.ShellCompDirectiveNoFileComp
			}
			if len(args) == 0 {
				var aliases []string
				for _, child := range current.Commands() {
					if !child.IsAvailableCommand() {
						continue
					}
					for _, alias := range child.Aliases {
						if strings.HasPrefix(alias, toComplete) {
							aliases = append(aliases, cobra.CompletionWithDesc(alias, child.Short))
						}
					}
				}
				if len(aliases) != 0 {
					return aliases, cobra.ShellCompDirectiveNoFileComp
				}
			}
			return nil, cobra.ShellCompDirectiveDefault
		}
		command.SetHelpFunc(func(command *cobra.Command, _ []string) {
			text := item.HelpDefault
			if os.Getenv("COLUMNS") == "80" || text == "" {
				text = item.Help
			}
			_, _ = io.WriteString(command.OutOrStdout(), text)
		})
		for _, flag := range item.Flags {
			if err := registerFlag(command.Flags(), flag); err != nil {
				return nil, fmt.Errorf("%s: %w", strings.Join(item.Path, " "), err)
			}
			if len(flag.Choices) != 0 {
				_ = command.RegisterFlagCompletionFunc(flag.Name, cobra.FixedCompletions(flag.Choices, cobra.ShellCompDirectiveNoFileComp))
			}
		}
		if err := registerFlag(command.Flags(), FlagSpec{Name: "help", Short: "h", Kind: "bool", Help: "Print help"}); err != nil {
			return nil, err
		}
		commands[strings.Join(item.Path, " ")] = command
	}
	root := commands[""]
	root.Version = version.Current().Version
	root.SetVersionTemplate("mf {{.Version}}\n")
	root.CompletionOptions.DisableDefaultCmd = true
	root.SetHelpCommand(&cobra.Command{Use: "help", Hidden: true, RunE: func(_ *cobra.Command, _ []string) error {
		return &parseFailure{kind: "unknown_command", value: "help"}
	}})
	root.PersistentPreRun = prepareCompletion
	for _, flag := range spec.GlobalFlags {
		if flag.Name == "help" || flag.Name == "version" {
			continue
		}
		if err := registerFlag(root.PersistentFlags(), flag); err != nil {
			return nil, err
		}
		if len(flag.Choices) != 0 {
			if err := root.RegisterFlagCompletionFunc(flag.Name, cobra.FixedCompletions(flag.Choices, cobra.ShellCompDirectiveNoFileComp)); err != nil {
				return nil, err
			}
		}
	}
	if err := registerFlag(root.PersistentFlags(), FlagSpec{Name: "version", Short: "V", Kind: "bool", Help: "Print version"}); err != nil {
		return nil, err
	}
	for _, item := range spec.Commands {
		if len(item.Path) != 0 {
			commands[strings.Join(item.Path[:len(item.Path)-1], " ")].AddCommand(commands[strings.Join(item.Path, " ")])
		}
	}
	return root, nil
}

type checkedValue struct {
	pflag.Value
	spec FlagSpec
	seen bool
}

func (value *checkedValue) Set(text string) error {
	if value.seen && !value.spec.Repeat && value.spec.Kind != "count" {
		return &parseFailure{kind: "duplicate", flag: value.spec}
	}
	if value.spec.Kind == "bool" || value.spec.Kind == "count" {
		if text != "__present__" {
			return &parseFailure{kind: "unexpected_value", value: text, flag: value.spec}
		}
		text = "true"
		if value.spec.Kind == "count" {
			text = "+1"
		}
	}
	if len(value.spec.Choices) != 0 && !slices.Contains(value.spec.Choices, text) {
		return &parseFailure{kind: "invalid_value", value: text, flag: value.spec}
	}
	if value.spec.Kind == "int" {
		if _, err := strconv.ParseInt(text, 10, value.spec.Bits); err != nil {
			detail := "invalid digit found in string"
			if errors.Is(err, strconv.ErrRange) {
				detail = "number too large to fit in target type"
				if _, err := strconv.ParseInt(text, 10, 64); err == nil {
					minimum := -(int64(1) << (value.spec.Bits - 2)) * 2
					detail = fmt.Sprintf("%s is not in %d..=%d", text, minimum, -(minimum + 1))
				}
			}
			return &parseFailure{kind: "invalid_value", value: text, flag: value.spec, detail: detail}
		}
	}
	if value.spec.Kind == "uint" {
		if _, err := strconv.ParseUint(strings.TrimPrefix(text, "+"), 10, value.spec.Bits); err != nil {
			detail := "invalid digit found in string"
			if errors.Is(err, strconv.ErrRange) {
				detail = "number too large to fit in target type"
			}
			return &parseFailure{kind: "invalid_value", value: text, flag: value.spec, detail: detail}
		}
		text = strings.TrimPrefix(text, "+")
	}
	if value.spec.Kind == "float" {
		if _, err := strconv.ParseFloat(text, 64); err != nil {
			return &parseFailure{kind: "invalid_value", value: text, flag: value.spec, detail: "invalid float literal"}
		}
	}
	if err := value.Value.Set(text); err != nil {
		return err
	}
	value.seen = true
	return nil
}

func registerFlag(flags *pflag.FlagSet, spec FlagSpec) error {
	defaultValue := ""
	if spec.Default != nil {
		defaultValue = *spec.Default
	}
	switch spec.Kind {
	case "bool":
		flags.BoolP(spec.Name, spec.Short, defaultValue == "true", spec.Help)
	case "count":
		flags.CountP(spec.Name, spec.Short, spec.Help)
	case "string":
		if spec.Repeat {
			flags.StringArrayP(spec.Name, spec.Short, nil, spec.Help)
		} else {
			flags.StringP(spec.Name, spec.Short, defaultValue, spec.Help)
		}
	case "int":
		value, err := strconv.ParseInt(zeroDefault(defaultValue), 10, spec.Bits)
		if err != nil {
			return err
		}
		flags.Int64P(spec.Name, spec.Short, value, spec.Help)
	case "uint":
		value, err := strconv.ParseUint(zeroDefault(defaultValue), 10, spec.Bits)
		if err != nil {
			return err
		}
		flags.Uint64P(spec.Name, spec.Short, value, spec.Help)
	case "float":
		value, err := strconv.ParseFloat(zeroDefault(defaultValue), 64)
		if err != nil {
			return err
		}
		flags.Float64P(spec.Name, spec.Short, value, spec.Help)
	default:
		return fmt.Errorf("unsupported flag kind %q", spec.Kind)
	}
	flag := flags.Lookup(spec.Name)
	flag.Hidden = spec.Hidden
	flag.Value = &checkedValue{Value: flag.Value, spec: spec}
	if spec.Kind == "bool" || spec.Kind == "count" {
		flag.NoOptDefVal = "__present__"
	}
	for _, alias := range spec.Aliases {
		flags.AddFlag(&pflag.Flag{Name: alias, Usage: spec.Help, Hidden: spec.Hidden, Value: flag.Value, DefValue: flag.DefValue, NoOptDefVal: flag.NoOptDefVal})
	}
	return nil
}

func zeroDefault(value string) string {
	if value == "" {
		return "0"
	}
	return value
}

func flagPresent(command *cobra.Command, name string) bool {
	flag := command.Flags().Lookup(name)
	if flag == nil {
		flag = command.InheritedFlags().Lookup(name)
	}
	if flag == nil {
		return false
	}
	if value, ok := flag.Value.(*checkedValue); ok {
		return value.seen
	}
	return flag.Changed
}

func validateCommand(command *cobra.Command, args []string) error {
	spec, err := LoadSpec()
	if err != nil {
		return err
	}
	path := strings.Fields(command.CommandPath())[1:]
	item, ok := spec.Lookup(path)
	if !ok {
		return &parseFailure{kind: "unknown_command", value: command.Name()}
	}
	if flagPresent(command, "quiet") && flagPresent(command, "verbose") {
		return output.Usage("'--verbose' cannot be used with '--quiet'", "")
	}
	var missing []string
	for _, flag := range append(append([]FlagSpec{}, item.Flags...), spec.GlobalFlags...) {
		present := flagPresent(command, flag.Name)
		if flag.Required && !present {
			missing = append(missing, flagLabel(flag))
		}
		if !present {
			continue
		}
		for _, other := range flag.Conflicts {
			if flagPresent(command, other) {
				otherSpec, _ := spec.Flag(path, "--"+other)
				return &parseFailure{kind: "conflict", flag: flag, other: flagLabel(otherSpec)}
			}
		}
		for _, other := range flag.Requires {
			if !flagPresent(command, other) {
				otherSpec, _ := spec.Flag(path, "--"+other)
				missing = append(missing, flagLabel(otherSpec))
			}
		}
	}
	position := 0
	for _, argument := range item.Arguments {
		if position == len(args) {
			if argument.Required {
				missing = append(missing, "<"+argument.Name+">")
			}
			continue
		}
		end := position + 1
		if argument.Repeat {
			end = len(args)
		}
		for _, value := range args[position:end] {
			if len(argument.Choices) > 0 && !slices.Contains(argument.Choices, value) {
				return &parseFailure{kind: "invalid_value", value: value, label: "<" + argument.Name + ">", flag: FlagSpec{Choices: argument.Choices}}
			}
		}
		position = end
	}
	if position < len(args) {
		kind := "unexpected_argument"
		if command.HasSubCommands() && len(item.Arguments) == 0 {
			kind = "unknown_command"
		}
		return &parseFailure{kind: kind, value: args[position]}
	}
	if len(missing) > 0 {
		return &parseFailure{kind: "missing", missing: missing}
	}
	if item.RequiredSubcommand && len(command.Commands()) > 0 {
		return &parseFailure{kind: "subcommand_required"}
	}
	return nil
}
