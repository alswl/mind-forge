package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/logging"
	"github.com/alswl/mind-forge/pkg/common/task"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/build"
	"github.com/alswl/mind-forge/pkg/services/session"
	"github.com/spf13/cobra"
)

func Run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	return runWithProcessIO(ctx, args, stdout, stderr, build.ProcessIO{Stdin: os.Stdin, Stdout: os.Stdout, Stderr: os.Stderr})
}

func runWithProcessIO(ctx context.Context, args []string, stdout, stderr io.Writer, processIO build.ProcessIO) int {
	cwd, cwdError := os.Getwd()
	environment := snapshotEnvironment()
	diagnosticFormat := "text"
	dispatched := false
	root, err := NewCommand(func(command *cobra.Command, item CommandSpec, positional []string) error {
		dispatched = true
		if boolFlag(command, "generate-manual") {
			catalogue, err := LoadSpec()
			if err != nil {
				return err
			}
			_, err = io.WriteString(stdout, Manual(catalogue))
			return err
		}
		if len(item.Path) > 0 && (item.Path[0] == "version" || item.Path[0] == "completion") {
			format := stringFlag(command, "output")
			if boolFlag(command, "json") {
				format = "json"
			}
			application := &app.Context{Format: format, CWD: cwd, Quiet: boolFlag(command, "quiet"), Color: !boolFlag(command, "no-color")}
			registration := commandRegistrations[strings.Join(item.Path, " ")]
			return registration.run(commandExecution{Command: command, Application: application, Args: positional, Stdout: stdout, Stderr: stderr, ProcessIO: processIO})
		}
		if cwdError != nil {
			return cwdError
		}
		originalProjectPresent := flagPresent(command, "project")
		settings, err := app.ResolveRuntime(runtimeInput(command, cwd, environment))
		diagnosticFormat = settings.Output
		if boolFlag(command, "json") {
			diagnosticFormat = "json"
		}
		if err != nil {
			return err
		}
		command.SetContext(withRuntime(command.Context(), settings))
		command.SetContext(logging.WithLogger(command.Context(), stderr, settings.Verbose > 0))
		application, err := app.NewContext(app.ContextOptions{ResolveOptions: app.ResolveOptions{CWD: cwd}, Runtime: &settings, JSON: boolFlag(command, "json")})
		if err != nil {
			return err
		}
		command.SetContext(task.WithObserver(command.Context(), invocationObserver(stdout, stderr, application, environment)))
		selection := runtimeSelection(application, item, positional, settings, originalProjectPresent)
		command.SetContext(withSelection(command.Context(), selection))
		positional = selection.Args
		if item.RepoRequired {
			if _, err := application.RequireRepo(); err != nil {
				return err
			}
		}
		catalogue, err := LoadSpec()
		if err != nil {
			return err
		}
		access := catalogue.Classify(item.Path, Modifiers{DryRun: boolFlag(command, "dry-run"), Check: boolFlag(command, "check"), Fix: boolFlag(command, "fix")})
		lock, err := session.Begin(command.Context(), application.RepoRoot, access == Write, access == Recovery)
		if err != nil {
			return err
		}
		defer lock.Close()

		if len(item.Path) >= 2 && item.Path[0] == "article" {
			if err := checkArticleLayout(application, optionalFlag(command, "project")); err != nil {
				return err
			}
		}
		registration := commandRegistrations[strings.Join(item.Path, " ")]
		return registration.run(commandExecution{Command: command, Application: application, Args: positional, Stdout: stdout, Stderr: stderr, Selection: selection, ProcessIO: processIO})
	})
	if err != nil {
		fmt.Fprintln(stderr, "internal error:", err)
		return 1
	}
	root.SetArgs(args)
	helpOutput := &checkedWriter{Writer: stdout}
	root.SetOut(helpOutput)
	root.SetErr(stderr)
	if len(args) == 0 {
		if err := root.Help(); err != nil || helpOutput.err != nil {
			return 1
		}
		return 0
	}
	selected, executeError := root.ExecuteContextC(ctx)
	err = executeError
	if err == nil {
		err = helpOutput.err
	}
	if err == nil {
		return 0
	}
	var status *commandExit
	if errors.As(err, &status) {
		return status.code
	}
	var parseError *parseFailure
	if errors.As(err, &parseError) {
		catalogue, loadError := LoadSpec()
		if loadError != nil {
			return 1
		}
		if selected == nil {
			selected = root
		}
		if cwdError == nil {
			settings, _ := app.ResolveRuntime(runtimeInput(selected, cwd, environment))
			selected.SetContext(withRuntime(ctx, settings))
		}
		return renderParseFailure(selected, catalogue, parseError, args, stderr)
	}
	format := diagnosticFormat
	if wantsJSON(args) {
		format = "json"
	}
	var failure *output.Error
	if !errors.As(err, &failure) {
		if !dispatched {
			failure = output.Usage(err.Error(), "")
			failure.Cause = err
		} else {
			failure = output.Internal(err)
		}
	}
	if err := output.RenderError(stderr, format, failure); err != nil {
		return 1
	}
	return failure.ExitCode()
}

type checkedWriter struct {
	io.Writer
	err error
}

func (writer *checkedWriter) Write(raw []byte) (int, error) {
	count, err := writer.Writer.Write(raw)
	if err == nil && count != len(raw) {
		err = io.ErrShortWrite
	}
	if writer.err == nil {
		writer.err = err
	}
	return count, err
}

func stringFlag(command *cobra.Command, name string) string {
	if name == "project" {
		if selection, ok := selectionFromCommand(command); ok {
			return selection.Project
		}
	}
	if settings, ok := runtimeFromCommand(command); ok {
		switch name {
		case "root":
			return settings.Root
		case "config":
			return settings.Config
		case "output":
			return settings.Output
		}
	}
	value, _ := command.Flags().GetString(name)
	return value
}

func boolFlag(command *cobra.Command, name string) bool {
	if settings, ok := runtimeFromCommand(command); ok {
		switch name {
		case "quiet":
			return settings.Quiet
		case "no-color":
			return settings.NoColor
		}
	}

	value, _ := command.Flags().GetBool(name)
	return value
}

func wantsJSON(args []string) bool {
	previousOutput := false
	for _, arg := range args {
		if arg == "--json" || previousOutput && arg == "json" || arg == "--output=json" || arg == "-o=json" || arg == "-ojson" {
			return true
		}
		previousOutput = arg == "--output" || arg == "-o"
	}
	return false
}

func Execute() int {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	return runWithProcessIO(ctx, os.Args[1:], os.Stdout, os.Stderr, build.ProcessIO{Stdin: os.Stdin, Stdout: os.Stdout, Stderr: os.Stderr})
}
