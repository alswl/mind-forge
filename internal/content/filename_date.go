package content

import (
	"strings"
	"time"
)

func LeadingFilenameDate(basename string) (time.Time, bool) {
	stem, _, _ := strings.Cut(basename, ".")
	if len(stem) < 10 {
		return time.Time{}, false
	}
	date, err := time.Parse("2006-01-02", stem[:10])
	return date, err == nil
}
