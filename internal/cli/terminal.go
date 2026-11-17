package cli

import (
	"io"
	"os"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/output"
)

func terminalCommand(application *app.Context, stdout io.Writer) error {
	env := map[string]string{}
	for _, name := range []string{"TERM", "TERM_PROGRAM", "COLORTERM", "TMUX", "NO_COLOR", "MF_FORCE_TTY", "MF_NO_HYPERLINKS", "MF_FORCE_HYPERLINKS"} {
		if value, ok := os.LookupEnv(name); ok {
			env[name] = value
		}
	}
	profile := output.BuildTerminalProfile(output.CapabilityInput{Env: env, Width: 200})
	if file, ok := stdout.(*os.File); ok {
		profile = output.ProbeTerminal(file)
	}
	report := output.TerminalDiagnostics(profile, application.Format, env)
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, application.Format, report)
	}
	return output.RenderRaw(stdout, application.Format, report.Text())
}
