package textcase

import (
	"encoding/json"
	"os"
	"testing"
)

func TestLowercasingMatchesFrozenRustUnicodeCorpus(t *testing.T) {
	raw, err := os.ReadFile("testdata/lowercase-legacy.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures [][2]string
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	for _, fixture := range fixtures {
		if actual := Lower(fixture[0]); actual != fixture[1] {
			t.Fatalf("lower(%q)=%q, want %q", fixture[0], actual, fixture[1])
		}
	}
}
