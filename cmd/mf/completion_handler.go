package main

import (
	"io"

	"github.com/spf13/cobra"
)

func prepareCompletion(command *cobra.Command, _ []string) {
	if command.Name() != cobra.ShellCompRequestCmd {
		return
	}
	// Cobra offers its hidden help command as a completion; mf exposes help flags.
	root := command.Root()
	for _, child := range root.Commands() {
		if child.Name() == "help" && child.Hidden {
			root.RemoveCommand(child)
		}
	}
}

func completionCommand(command *cobra.Command, shell string, stdout io.Writer) error {
	root := command.Root()
	switch shell {
	case "bash":
		return root.GenBashCompletionV2(stdout, true)
	case "zsh":
		return root.GenZshCompletion(stdout)
	case "fish":
		return root.GenFishCompletion(stdout, true)
	case "powershell":
		return root.GenPowerShellCompletionWithDesc(stdout)
	case "elvish":
		_, err := io.WriteString(stdout, `# Completion for mf; suggestions come from the active command tree.
use str
set edit:completion:arg-completer[mf] = {|@words|
  var candidates = [(mf __completeNoDesc (all $words[1..]) 2>/dev/null)]
  for candidate $candidates {
    if (not (str:has-prefix $candidate ':')) {
      put $candidate
    }
  }
}
`)
		return err
	default:
		return nil // The argument validator rejects unsupported shells.
	}
}
