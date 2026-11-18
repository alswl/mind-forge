//go:build darwin || linux

package store

import (
	"fmt"
	"os"
	"syscall"
)

func fileOwner(info os.FileInfo) (int, int, error) {
	metadata, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return 0, 0, fmt.Errorf("unsupported file ownership metadata")
	}
	return int(metadata.Uid), int(metadata.Gid), nil
}
