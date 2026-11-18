package output

import (
	"encoding/json"
	"os"
	"testing"
)

func TestWidthMatchesPinnedRustFixture(t *testing.T) {
	raw, err := os.ReadFile("testdata/width-legacy.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []struct {
		Text  string `json:"text"`
		Width int    `json:"width"`
	}
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	for _, fixture := range fixtures {
		if got := DisplayWidth(fixture.Text); got != fixture.Width {
			t.Errorf("width(%q)=%d, want %d", fixture.Text, got, fixture.Width)
		}
	}
}
