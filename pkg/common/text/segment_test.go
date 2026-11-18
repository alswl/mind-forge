package text

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"hash"
	"os"
	"reflect"
	"strings"
	"testing"
)

type referenceCase struct {
	Input  string   `json:"input"`
	Tokens []string `json:"tokens"`
	Spans  []Span   `json:"spans"`
	Pinyin string   `json:"pinyin"`
}

func referenceCases(t *testing.T) []referenceCase {
	t.Helper()
	b, err := os.ReadFile("testdata/cases.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []referenceCase
	if err := json.Unmarshal(b, &cases); err != nil {
		t.Fatal(err)
	}
	return cases
}

func TestFrozenSegmentation(t *testing.T) {
	for _, c := range referenceCases(t) {
		t.Run(c.Input, func(t *testing.T) {
			got := Cut(c.Input)
			if !reflect.DeepEqual(got, c.Tokens) {
				t.Fatalf("tokens: got %q, want %q", got, c.Tokens)
			}
			s := Segment(c.Input)
			if !reflect.DeepEqual(s.Spans, c.Spans) {
				t.Fatalf("spans: got %+v, want %+v", s.Spans, c.Spans)
			}
			if strings.Join(got, "") != c.Input {
				t.Fatal("tokens changed source bytes")
			}
			for _, span := range c.Spans {
				if !s.Aligned(span.Start, span.Len) || !s.Exact(span.Start, span.Len) {
					t.Fatalf("token missing from boundaries: %+v", span)
				}
			}
		})
	}
}

func TestSegmentationBoundaryPolicies(t *testing.T) {
	s := Segment("缩小文件")
	if s.Aligned(3, 6) {
		t.Fatal("cross-token substring accepted")
	}
	if !s.Aligned(0, 12) || s.Exact(0, 12) {
		t.Fatal("edge alignment must differ from exact token")
	}
	if !Segment("").Aligned(0, 0) || Segment("").Exact(0, 0) {
		t.Fatal("empty input boundary")
	}
	if s.Aligned(-1, 3) || s.Aligned(0, -1) {
		t.Fatal("negative span accepted")
	}
}

func digestBytes(h hash.Hash, b []byte) {
	var size [8]byte
	binary.LittleEndian.PutUint64(size[:], uint64(len(b)))
	h.Write(size[:])
	h.Write(b)
}

func TestFrozenDictionaryCorpus(t *testing.T) {
	data := unpack(dictionaryData)
	h := sha256.New()
	count := 0
	for _, line := range strings.Split(string(data), "\n") {
		fields := strings.Fields(line)
		if len(fields) == 0 {
			continue
		}
		word := fields[0]
		digestBytes(h, []byte(word))
		tokens := Cut(word)
		var size [8]byte
		binary.LittleEndian.PutUint64(size[:], uint64(len(tokens)))
		h.Write(size[:])
		for _, token := range tokens {
			digestBytes(h, []byte(token))
		}
		count++
	}
	if count != 349045 {
		t.Fatalf("dictionary input count %d", count)
	}
	if got := hex.EncodeToString(h.Sum(nil)); got != "0cf02544c327165726d91319a84fcac4694e0a803cff52a88e16631714819d1e" {
		t.Fatalf("dictionary token digest %s", got)
	}
}
