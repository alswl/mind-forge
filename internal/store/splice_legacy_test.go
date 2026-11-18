package store

import "testing"

func TestLegacyYAMLBlockExtraction(t *testing.T) {
	t.Run("extract_top_level_key_returns_the_key_s_own_block", func(t *testing.T) {
		raw := []byte("project: alpha\nsources:\n  - name: a\nterms:\n  - term: API\n")
		start, end, ok := topLevelSpan(raw, "sources")
		if !ok || string(raw[start:end]) != "sources:\n  - name: a\n" {
			t.Fatalf("source block = %d:%d, found=%v", start, end, ok)
		}
	})
	t.Run("extract_top_level_key_is_none_when_absent", func(t *testing.T) {
		if _, _, ok := topLevelSpan([]byte("project: alpha\nterms:\n  - term: API\n"), "sources"); ok {
			t.Fatal("absent source block found")
		}
	})
}
