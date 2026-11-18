package content

import (
	"os"
	"syscall"
	"time"
)

func projectCreatedAt(path string) string {
	info, err := os.Stat(path)
	if err != nil {
		return ""
	}
	stamp := info.ModTime()
	if stat, ok := info.Sys().(*syscall.Stat_t); ok && stat.Birthtimespec.Sec >= 0 {
		stamp = time.Unix(stat.Birthtimespec.Sec, 0)
	}
	if stamp.Unix() < 0 {
		return ""
	}
	return stamp.UTC().Truncate(time.Second).Format(time.RFC3339)
}
