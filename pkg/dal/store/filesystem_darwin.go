package store

import (
	"bytes"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"golang.org/x/sys/unix"
)

func CheckLocalFilesystem(root string) error {
	var info unix.Statfs_t
	if err := unix.Statfs(root, &info); err != nil {
		return err
	}
	name := string(bytes.TrimRight(info.Fstypename[:], "\x00"))
	if name != "apfs" && name != "hfs" {
		return &output.Error{Kind: "unsupported_filesystem", Message: "only known local APFS/HFS/ext/XFS/Btrfs/tmpfs filesystems are supported", Lifecycle: true}
	}
	return nil
}
