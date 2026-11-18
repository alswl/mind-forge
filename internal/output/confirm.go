package output

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"
)

type ConfirmArgs struct {
	VerbLabel, Kind, Identity string
	Yes, Force                bool
}

func RequireConfirmation(args ConfirmArgs, stderr io.Writer, openTerminal func() (io.ReadCloser, error)) error {
	if args.Yes || args.Force {
		return nil
	}
	switch PromptConfirmation(fmt.Sprintf("Confirm %s of %s \"%s\"? [y/N] ", args.VerbLabel, args.Kind, args.Identity), stderr, openTerminal) {
	case Confirmed:
		return nil
	case NotTTY:
		return Usage(fmt.Sprintf("destructive operation requires confirmation for '%s'", args.Identity), "pass --yes to confirm")
	}

	return Usage(args.VerbLabel+" aborted", "")
}

type ConfirmOutcome uint8

const (
	Aborted ConfirmOutcome = iota
	Confirmed
	NotTTY
)

func PromptConfirmation(prompt string, stderr io.Writer, openTerminal func() (io.ReadCloser, error)) ConfirmOutcome {
	if openTerminal == nil {
		openTerminal = func() (io.ReadCloser, error) { return os.Open("/dev/tty") }
	}
	terminal, err := openTerminal()
	if err != nil {
		return NotTTY
	}
	defer terminal.Close()
	_, _ = fmt.Fprint(stderr, prompt)
	if flusher, ok := stderr.(interface{ Flush() error }); ok {
		_ = flusher.Flush()
	}
	answer, err := bufio.NewReader(terminal).ReadString('\n')
	if err == nil || err == io.EOF && len(answer) > 0 {
		switch strings.ToLower(strings.TrimSpace(answer)) {
		case "y", "yes":
			return Confirmed
		}
	}
	return Aborted
}
