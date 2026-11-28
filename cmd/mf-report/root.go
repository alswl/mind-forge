package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/reportcli"
	"github.com/alswl/mind-forge/internal/version"
	core "github.com/alswl/mind-forge/pkg/services/report"
	"github.com/spf13/cobra"
)

func NewCommand() *cobra.Command {
	root := &cobra.Command{
		Use: "mf-report", Short: "Work with Mind Forge reports",
		Version: version.Current().Version, SilenceErrors: true, SilenceUsage: true,
	}
	root.CompletionOptions.DisableDefaultCmd = true
	root.PersistentFlags().String("registry-root", "", "Mind Repo directory containing minds.yaml")
	root.PersistentFlags().String("workdir", ".", "Report working directory")
	root.PersistentFlags().Bool("json", false, "Write a JSON envelope")
	root.PersistentFlags().StringP("output", "o", "text", "Output format (text or json)")
	root.PersistentFlags().BoolP("quiet", "q", false, "Silence non-error output")
	root.PersistentPreRunE = func(command *cobra.Command, _ []string) error {
		if value := stringFlag(command, "output"); value != "text" && value != "json" {
			return output.Usage(fmt.Sprintf("unsupported output format %q", value), "use text or json")
		}
		return nil
	}
	root.AddCommand(newBuildCommand(), newCatalogCommand(), newListCommand(), newServeCommand(), newOpenAPICommand())
	return root
}

func Run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	root := NewCommand()
	dispatched := false
	var trackDispatch func(*cobra.Command)
	trackDispatch = func(command *cobra.Command) {
		for _, child := range command.Commands() {
			if action := child.RunE; action != nil {
				child.RunE = func(command *cobra.Command, args []string) error {
					dispatched = true
					return action(command, args)
				}
			}
			trackDispatch(child)
		}
	}
	trackDispatch(root)
	root.SetArgs(args)
	root.SetOut(stdout)
	root.SetErr(stderr)
	command, err := root.ExecuteContextC(ctx)
	if err == nil {
		return 0
	}
	var failure *output.Error
	if !errors.As(err, &failure) {
		if dispatched {
			failure = output.Internal(err)
		} else {
			failure = output.Usage(err.Error(), "")
		}
	}
	if err := output.RenderNamedError(stderr, format(command), "mf-report", failure); err != nil {
		return 1
	}
	return failure.ExitCode()
}

func Execute() int {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	return Run(ctx, os.Args[1:], os.Stdout, os.Stderr)
}

func stringFlag(command *cobra.Command, name string) string {
	value, _ := command.Flags().GetString(name)
	return value
}

func format(command *cobra.Command) string {
	if yes, _ := command.Flags().GetBool("json"); yes || stringFlag(command, "output") == "json" {
		return "json"
	}
	return "text"
}

func reportResult(command *cobra.Command, value any, err error) error {
	quiet, _ := command.Flags().GetBool("quiet")
	return reportcli.Result(command.OutOrStdout(), "mf-report", format(command), quiet, value, err)
}

func serviceFor(command *cobra.Command) (core.Service, error) {
	return reportcli.Service(stringFlag(command, "registry-root"), stringFlag(command, "worker"), command.ErrOrStderr())
}

func scopeFor(command *cobra.Command) (core.Scope, error) {
	return core.StandaloneScope(stringFlag(command, "workdir"))
}
