package output

import (
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/pkg/common/task"
)

type ProgressInput struct {
	StdoutIsTTY, StderrIsTTY bool
	Format                   string
	Quiet                    bool
	Term                     string
}

func ProgressEnabled(input ProgressInput) bool {
	return input.StdoutIsTTY && input.StderrIsTTY && input.Format == "text" && !input.Quiet && input.Term != "dumb"
}

func ProgressText(stage task.Stage) string {
	plain := func(value string) string {
		return strings.Map(func(r rune) rune {
			if unicode.IsControl(r) {
				return -1
			}
			return r
		}, value)
	}
	text := plain(stage.Operation) + ": " + plain(stage.Name)
	if stage.Label != "" {
		text += " (" + plain(stage.Label) + ")"
	}
	return text + "\n"
}
