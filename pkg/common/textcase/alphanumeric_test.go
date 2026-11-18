package textcase

import "testing"

func TestFrozenAlphanumericIncludesNewerScriptsAndAlphabeticMarks(t *testing.T) {
	for _, char := range []rune{'a', '0', '机', '\u0345', '\U000105C0'} {
		if !IsAlphanumeric(char) {
			t.Errorf("expected alphanumeric U+%04X", char)
		}
	}
	for _, char := range []rune{'-', ' ', '\u0301', -1, 0x110000} {
		if IsAlphanumeric(char) {
			t.Errorf("unexpected alphanumeric U+%04X", char)
		}
	}
}
