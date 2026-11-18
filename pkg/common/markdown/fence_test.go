package markdown

import "testing"

func TestFenceMarkersAndLength(t *testing.T) {
	var tracker FenceTracker
	lines := []struct {
		text   string
		inside bool
	}{{"# Heading", false}, {"````md", true}, {"```", true}, {"## Literal", true}, {"~~~~", true}, {"`````", true}, {"## Real", false}, {"    ```", false}, {"~~~lang", false}, {"~~~", true}, {"   ~~~ ", true}, {"text", false}}
	for _, line := range lines {
		if got := tracker.ProcessLine(line.text); got != line.inside {
			t.Errorf("%q inside=%v want %v", line.text, got, line.inside)
		}
	}
}
