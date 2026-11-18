package output

import (
	"os"
	"testing"
	"time"
)

// A pipe matches the original unit runner's nonterminal stdout. Interactive
// terminal and terminfo process acceptance belongs to the external harness.
func TestLegacyProfileProbeBudget(t *testing.T) {
	t.Setenv("MF_FORCE_TTY", "")
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	defer read.Close()
	defer write.Close()
	t.Run("build_profile_is_fast", func(t *testing.T) {
		start := time.Now()
		profile := ProbeTerminal(write)
		elapsed := time.Since(start)
		if profile.StdoutIsTTY {
			t.Fatal("pipe incorrectly classified as terminal")
		}
		if elapsed.Milliseconds() >= 100 {
			t.Fatalf("profile probe took %v, want under100ms", elapsed)
		}
	})
	t.Run("build_profile_repeated_is_stable", func(t *testing.T) {
		start := time.Now()
		for range 10 {
			ProbeTerminal(write)
		}
		elapsed := time.Since(start)
		if elapsed.Milliseconds() >= 200 {
			t.Fatalf("10profile probes took %v, want under200ms", elapsed)
		}
	})
}
