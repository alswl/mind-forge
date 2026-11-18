package text

import (
	"math"
	"strconv"
	"strings"
	"sync"
	"unicode/utf8"
)

// Span measures source bytes, matching the offsets used by term corrections.
type Span struct {
	Start int `json:"start"`
	Len   int `json:"len"`
}

type Segmentation struct {
	Spans      []Span
	boundaries map[int]bool
	exact      map[Span]bool
}

func Segment(input string) Segmentation {
	s := Segmentation{Spans: []Span{}, boundaries: map[int]bool{0: true}, exact: make(map[Span]bool)}
	pos := 0
	for _, token := range Cut(input) {
		span := Span{Start: pos, Len: len(token)}
		s.Spans = append(s.Spans, span)
		s.exact[span] = true
		pos += len(token)
		s.boundaries[pos] = true
	}
	return s
}

func (s Segmentation) Aligned(start, length int) bool {
	return start >= 0 && length >= 0 && start <= math.MaxInt-length && s.boundaries[start] && s.boundaries[start+length]
}

func (s Segmentation) Exact(start, length int) bool {
	return s.exact[Span{Start: start, Len: length}]
}

type segmentModel struct {
	// -1 denotes a prefix without its own dictionary record; zero is a real word.
	freq     map[string]int
	logTotal float64
	hmm      hmmModel
}

var defaultModel = sync.OnceValue(func() *segmentModel {
	m := &segmentModel{freq: make(map[string]int)}
	for _, line := range strings.Split(string(unpack(dictionaryData)), "\n") {
		fields := strings.Fields(line)
		if len(fields) == 0 {
			continue
		}
		if len(fields) < 2 {
			panic("invalid embedded dictionary row")
		}
		freq, err := strconv.Atoi(fields[1])
		if err != nil {
			panic(err)
		}
		word := fields[0]
		for i := range word {
			if i > 0 {
				if _, ok := m.freq[word[:i]]; !ok {
					m.freq[word[:i]] = -1
				}
			}
		}
		m.freq[word] = freq
	}
	total := 0
	for _, n := range m.freq {
		if n >= 0 {
			total += n
		}
	}
	m.logTotal = math.Log(float64(total))
	m.hmm = loadHMM()
	return m
})

// Cut uses the frozen default dictionary, maximum-probability DAG and HMM for
// unknown words. The returned slices retain the original UTF-8 bytes.
func Cut(input string) []string {
	words := []string{}
	m := defaultModel()
	for start := 0; start < len(input); {
		r, size := utf8.DecodeRuneInString(input[start:])
		if !defaultBlockRune(r) {
			if r == '\r' && start+size < len(input) && input[start+size] == '\n' {
				size++
			}
			words = append(words, input[start:start+size])
			start += size
			continue
		}
		end := start + size
		for end < len(input) {
			r, size = utf8.DecodeRuneInString(input[end:])
			if !defaultBlockRune(r) {
				break
			}
			end += size
		}
		words = m.cutDAG(input[start:end], words)
		start = end
	}
	return words
}

func defaultBlockRune(r rune) bool {
	if r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || strings.ContainsRune("+#&._%-", r) {
		return true
	}
	for _, span := range [][2]rune{{0x3400, 0x4dbf}, {0x4e00, 0x9fff}, {0xf900, 0xfaff}, {0x20000, 0x2a6df}, {0x2a700, 0x2b73f}, {0x2b740, 0x2b81f}, {0x2b820, 0x2ceaf}, {0x2ceb0, 0x2ebef}, {0x2f800, 0x2fa1f}} {
		if r >= span[0] && r <= span[1] {
			return true
		}
	}
	return false
}

func runeOffsets(s string) []int {
	positions := make([]int, 0, utf8.RuneCountInString(s)+1)
	for i := range s {
		positions = append(positions, i)
	}
	return append(positions, len(s))
}

func (m *segmentModel) cutDAG(s string, words []string) []string {
	pos := runeOffsets(s)
	scores := make([]float64, len(s)+1)
	next := make([]int, len(s))
	for i := len(pos) - 2; i >= 0; i-- {
		start := pos[i]
		best := math.Inf(-1)
		bestEnd := 0
		for j := i + 1; j < len(pos); j++ {
			end := pos[j]
			freq, ok := m.freq[s[start:end]]
			if !ok {
				break
			}
			if freq < 0 {
				continue
			}
			score := math.Log(float64(freq)) - m.logTotal + scores[end]
			if score > best || score == best && end > bestEnd {
				best = score
				bestEnd = end
			}
		}
		if bestEnd == 0 {
			bestEnd = pos[i+1]
			best = -m.logTotal + scores[bestEnd]
		}
		scores[start] = best
		next[start] = bestEnd
	}
	left := -1
	flush := func(end int) {
		if left < 0 {
			return
		}
		buffer := s[left:end]
		freq, exists := m.freq[buffer]
		if utf8.RuneCountInString(buffer) == 1 {
			words = append(words, buffer)
		} else if !exists || freq < 0 {
			words = m.hmm.cut(buffer, words)
		} else {
			p := runeOffsets(buffer)
			for i := 0; i < len(p)-1; i++ {
				words = append(words, buffer[p[i]:p[i+1]])
			}
		}
		left = -1
	}
	for start := 0; start < len(s); {
		end := next[start]
		if utf8.RuneCountInString(s[start:end]) == 1 {
			if left < 0 {
				left = start
			}
		} else {
			flush(start)
			words = append(words, s[start:end])
		}
		start = end
	}
	flush(len(s))
	return words
}
