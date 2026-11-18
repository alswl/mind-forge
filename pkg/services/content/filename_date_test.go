package content

import (
	"testing"
	"time"
)

func TestLeadingFilenameDateFrozenCases(t *testing.T) {
	for _, test := range []struct {
		name  string
		valid bool
	}{
		{"2026-05-15-launch.md", true}, {"launch.md", false}, {"2026-13-40-bad.md", false}, {"abc.md", false},
		{"2024-02-29-note.md", true}, {"2026-02-29-note.md", false}, {"2026-05.15-note.md", false},
	} {
		date, ok := LeadingFilenameDate(test.name)
		if ok != test.valid {
			t.Fatalf("%s: %v %v", test.name, date, ok)
		}
		if test.name == "2026-05-15-launch.md" && !date.Equal(time.Date(2026, 5, 15, 0, 0, 0, 0, time.UTC)) {
			t.Fatal(date)
		}
	}
}
