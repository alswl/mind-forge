//go:build darwin || linux

package lifecycle

import (
	"os"
	"syscall"
)

type properties struct {
	device, links, inode uint64
	uid, gid             uint32
	mode                 uint32
}

func fileProperties(info os.FileInfo) (properties, error) {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return properties{}, rejected("unsupported_filesystem", "Unix file metadata unavailable")
	}
	return properties{device: uint64(stat.Dev), links: uint64(stat.Nlink), inode: uint64(stat.Ino), uid: stat.Uid, gid: stat.Gid, mode: uint32(stat.Mode) & 0o7777}, nil
}
