package text

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"testing"
	"unicode/utf8"
)

func TestFrozenPinyinStrings(t *testing.T) {
	for _, c := range referenceCases(t) {
		if got := Pinyin(c.Input); got != c.Pinyin {
			t.Errorf("%q: got %q, want %q", c.Input, got, c.Pinyin)
		}
	}
}

func TestFrozenUnicodePinyin(t *testing.T) {
	h := sha256.New()
	known := 0
	for r := rune(0); r <= utf8.MaxRune; r++ {
		if !utf8.ValidRune(r) {
			continue
		}
		plain := Pinyin(string(r))
		if plain != "" {
			known++
		}
		var cp [4]byte
		binary.LittleEndian.PutUint32(cp[:], uint32(r))
		h.Write(cp[:])
		digestBytes(h, []byte(plain))
	}
	if known != 41497 {
		t.Fatalf("known rune count %d", known)
	}
	if got := hex.EncodeToString(h.Sum(nil)); got != "26ee7e7bcbd612b9f2200a9a8dd808bd1a304c325ff054a866feff8180b8d75b" {
		t.Fatalf("Unicode pinyin digest %s", got)
	}
}
