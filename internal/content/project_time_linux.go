package content

import (
	"os"
	"time"

	"golang.org/x/sys/unix"
)

func projectCreatedAt(path string) string {
	info, err := os.Stat(path)
	if err != nil {
		return ""
	}
	stamp := info.ModTime()
	var stat unix.Statx_t
	if err := unix.Statx(unix.AT_FDCWD, path, 0, unix.STATX_BTIME, &stat); err == nil && stat.Mask&unix.STATX_BTIME != 0 {
		stamp = time.Unix(stat.Btime.Sec, 0)
	}
	if stamp.Unix() < 0 {
		return ""
	}
	return stamp.UTC().Truncate(time.Second).Format(time.RFC3339)
}
