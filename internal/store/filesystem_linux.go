package store

import (
	"github.com/alswl/mind-forge/internal/output"
	"golang.org/x/sys/unix"
)

func CheckLocalFilesystem(root string) error {
	var info unix.Statfs_t
	if err := unix.Statfs(root, &info); err != nil {
		return err
	}
	switch uint64(info.Type) {
	case 0xef53, 0x9123683e, 0x58465342, 0x01021994:
		return nil
	default:
		return &output.Error{Kind: "unsupported_filesystem", Message: "only known local APFS/HFS/ext/XFS/Btrfs/tmpfs filesystems are supported", Lifecycle: true}
	}
}
