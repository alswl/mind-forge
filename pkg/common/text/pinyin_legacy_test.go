package text

import "testing"

func TestLegacyPinyinSynthetic(t *testing.T) {
	t.Run("three_syllable_original", func(t *testing.T) {
		if got := Pinyin("时刻表"); got != "shi-ke-biao" {
			t.Fatalf("pinyin = %q, want shi-ke-biao", got)
		}
	})
	t.Run("three_syllable_variant", func(t *testing.T) {
		if got := Pinyin("实刻表"); got != "shi-ke-biao" {
			t.Fatalf("pinyin = %q, want shi-ke-biao", got)
		}
	})
	t.Run("homophone_pair", func(t *testing.T) {
		if first, second := Pinyin("时刻"), Pinyin("实刻"); first != second || first == "" {
			t.Fatalf("homophone pinyin differs: %q and %q", first, second)
		}
	})
	t.Run("non_cjk_returns_empty", func(t *testing.T) {
		if got := Pinyin("hello"); got != "" {
			t.Fatalf("non-CJK pinyin = %q, want empty", got)
		}
	})
}
