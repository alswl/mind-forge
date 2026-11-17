package cli

import (
	"context"
	"errors"
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"github.com/alswl/mind-forge/internal/version"
	"github.com/spf13/cobra"
)

func Run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
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
		options := app.ContextOptions{
			ResolveOptions: app.ResolveOptions{Root: stringFlag(command, "root"), Config: stringFlag(command, "config")},
			Project:        stringFlag(command, "project"), Format: stringFlag(command, "output"),
			JSON: boolFlag(command, "json"), NoColor: boolFlag(command, "no-color"), Quiet: boolFlag(command, "quiet"),
		}
		verbose, _ := command.Flags().GetCount("verbose")
		options.Verbose = uint(verbose)
		application, err := app.NewContext(options)
		if err != nil {
			return err
		}
		if len(positional) > 0 && hasPathSelector(item.Path) {
			if project, relative, adopted := app.SelectProjectFromPath(application.RepoRoot, application.CWD, positional[0], optionalFlag(command, "project") != nil); adopted {
				positional = append([]string(nil), positional...)
				positional[0] = relative
				if err := command.Flags().Set("project", project); err != nil {
					return err
				}
			}
		}
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
		if application.RepoRoot != "" && access != Recovery {
			if access == Write {
				lock, err := store.AcquireWriteLock(application.RepoRoot)
				if err != nil {
					return err
				}
				defer lock.Close()
			}
			if err := (lifecycle.Engine{}).CheckRecovery(application.RepoRoot); err != nil {
				return err
			}
		}
		if len(item.Path) >= 2 && item.Path[0] == "article" {
			if err := checkArticleLayout(application, optionalFlag(command, "project")); err != nil {
				return err
			}
		}
		if len(item.Path) >= 1 && item.Path[0] == "prompt" {
			return promptCommand(command, application, positional, stdout)
		}
		if len(item.Path) >= 1 && item.Path[0] == "thinking" {
			return thinkingCommand(command, application, positional, stdout)
		}
		if len(item.Path) >= 1 && item.Path[0] == "render" {
			return renderCommand(command, application, positional, stdout)
		}
		if len(item.Path) >= 1 && item.Path[0] == "publish" {
			return publishCommand(command, application, positional, stdout, stderr)
		}
		if command.HasSubCommands() || len(item.Path) == 0 {
			return command.Help()
		}
		if len(item.Path) >= 2 && item.Path[0] == "term" {
			return termCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 1 && item.Path[0] == "build" {
			return buildCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "config" {
			return configCommand(command, application, stdout)
		}
		if len(item.Path) == 2 && item.Path[0] == "article" && command.Name() == "convert" {
			return articleConvertCommand(command, application, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "article" && (command.Name() == "rename" || command.Name() == "move" || command.Name() == "remove") {
			return articleMutationCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "article" && (command.Name() == "index" || command.Name() == "lint") {
			return articleIndexCommand(command, application, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "article" && (command.Name() == "list" || command.Name() == "show") {
			return articleReadCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "article" && (command.Name() == "new" || command.Name() == "update") {
			return articleCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 3 && item.Path[0] == "article" && item.Path[1] == "block" {
			return articleBlockCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "project" {
			return projectCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "asset" {
			return assetCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "source" {
			return sourceCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "additional" {
			return additionalCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && item.Path[0] == "lifecycle" {
			return lifecycleCommand(command, application, positional, stdout, stderr)
		}
		if len(item.Path) == 2 && (item.Path[0] == "mind" || item.Path[0] == "output") {
			return mindCommand(command, application, positional, stdout)
		}
		if len(item.Path) == 1 && item.Path[0] == "init" {
			target := "."
			if len(positional) > 0 {
				target = positional[0]
			}
			report, err := content.InitializeRepository(target)
			if err != nil {
				return err
			}
			if application.Quiet {
				return nil
			}
			return output.RenderSuccess(stdout, application.Format, report)
		}
		if len(item.Path) == 1 && item.Path[0] == "completion" {
			return completionCommand(command, positional[0], stdout)
		}
		if len(item.Path) == 1 && item.Path[0] == "version" {
			if application.Quiet {
				return nil
			}
			metadata := version.Current()
			if application.Format == "json" {
				return output.RenderSuccess(stdout, application.Format, metadata)
			}
			return output.RenderRaw(stdout, application.Format, metadata.Text())
		}
		return output.NotImplemented(strings.Join(item.Path, " ")+" is not yet implemented", "")
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
		return renderParseFailure(selected, catalogue, parseError, args, stderr)
	}
	format := "text"
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
	value, _ := command.Flags().GetString(name)
	return value
}

func boolFlag(command *cobra.Command, name string) bool {
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
