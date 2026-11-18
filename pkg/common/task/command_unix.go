//go:build darwin || linux

package task

import (
	"context"
	"os"
	"os/exec"
	"syscall"
	"time"
)

// Command cancels the process group, including tools launched by a shell stage.
func Command(ctx context.Context, name string, args ...string) *exec.Cmd {
	command := exec.CommandContext(Context(ctx), name, args...)
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	command.Cancel = func() error {
		if command.Process == nil {
			return os.ErrProcessDone
		}
		err := syscall.Kill(-command.Process.Pid, syscall.SIGKILL)
		if err == syscall.ESRCH {
			return os.ErrProcessDone
		}
		return err
	}
	command.WaitDelay = time.Second
	return command
}
