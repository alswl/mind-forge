package main

import (
	"fmt"
	"strings"

	"github.com/alswl/mind-forge/internal/version"
	"github.com/spf13/cobra"
)

func NewCommand(handler Handler) (*cobra.Command, error) {
	spec, err := LoadSpec()
	if err != nil {
		return nil, err
	}
	commands := make(map[string]*cobra.Command, len(spec.Commands))
	for _, item := range spec.Commands {
		key := strings.Join(item.Path, " ")
		registration, ok := commandRegistrations[key]
		if !ok {
			return nil, fmt.Errorf("command %q has no registered factory", key)
		}
		command, err := registration.build(item, handler)
		if err != nil {
			return nil, err
		}
		commands[key] = command
	}
	if len(commands) != len(commandRegistrations) {
		return nil, fmt.Errorf("registered commands do not match the command catalogue")
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
			key := strings.Join(item.Path, " ")
			registration := commandRegistrations[key]
			expectedParent := strings.Join(item.Path[:len(item.Path)-1], " ")
			if registration.parent != expectedParent || commands[registration.parent] == nil {
				return nil, fmt.Errorf("invalid parent for command %q", key)
			}
			commands[registration.parent].AddCommand(commands[key])
		}
	}
	return root, nil
}

func init() {
	registerCommand("", "", newRootCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newRootCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "mf"}
	return configureCommand(command, item, handler)
}
