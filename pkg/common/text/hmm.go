package text

import (
	"math"
	"regexp"
	"strconv"
	"strings"
)

const minProbability = -3.14e100

type hmmModel struct {
	initial    [4]float64
	transition [4][4]float64
	emission   [4]map[rune]float64
}

var hmmSkip = regexp.MustCompile(`[a-zA-Z0-9]+(?:.\p{Nd}+)?%?`)

func loadHMM() hmmModel {
	var m hmmModel
	var lines []string
	for _, line := range strings.Split(string(unpack(hmmData)), "\n") {
		if line != "" && !strings.HasPrefix(line, "#") {
			lines = append(lines, line)
		}
	}
	parse := func(s string) float64 {
		n, err := strconv.ParseFloat(s, 64)
		if err != nil {
			panic(err)
		}
		return n
	}
	for i, s := range strings.Fields(lines[0]) {
		m.initial[i] = parse(s)
	}
	for i := 0; i < 4; i++ {
		for j, s := range strings.Fields(lines[i+1]) {
			m.transition[i][j] = parse(s)
		}
		m.emission[i] = make(map[rune]float64)
		for _, field := range strings.Split(lines[i+5], ",") {
			key, value, ok := strings.Cut(field, ":")
			if !ok {
				panic("invalid embedded HMM emission")
			}
			m.emission[i][[]rune(key)[0]] = parse(value)
		}
	}
	return m
}

func (m *hmmModel) cut(s string, words []string) []string {
	pos := runeOffsets(s)
	runes := []rune(s)
	for i := 0; i < len(runes); {
		han := runes[i] >= 0x4e00 && runes[i] <= 0x9fd5
		j := i + 1
		for j < len(runes) && (runes[j] >= 0x4e00 && runes[j] <= 0x9fd5) == han {
			j++
		}
		block := s[pos[i]:pos[j]]
		if han && j-i > 1 {
			words = m.viterbi(block, words)
		} else if han {
			words = append(words, block)
		} else {
			start := 0
			for _, match := range hmmSkip.FindAllStringIndex(block, -1) {
				if match[0] > start {
					words = append(words, block[start:match[0]])
				}
				words = append(words, block[match[0]:match[1]])
				start = match[1]
			}
			if start < len(block) {
				words = append(words, block[start:])
			}
		}
		i = j
	}
	return words
}

func (m *hmmModel) viterbi(s string, words []string) []string {
	runes := []rune(s)
	pos := runeOffsets(s)
	v := make([][4]float64, len(runes))
	prev := make([][4]int, len(runes))
	allowed := [4][2]int{{1, 3}, {0, 2}, {2, 0}, {3, 1}}
	emit := func(state int, r rune) float64 {
		p, ok := m.emission[state][r]
		if !ok {
			return minProbability
		}
		return p
	}
	for y := 0; y < 4; y++ {
		v[0][y] = m.initial[y] + emit(y, runes[0])
	}
	for t := 1; t < len(runes); t++ {
		for y := 0; y < 4; y++ {
			best := math.Inf(-1)
			state := -1
			for _, p := range allowed[y] {
				score := v[t-1][p] + m.transition[p][y] + emit(y, runes[t])
				if score > best || score == best && p > state {
					best = score
					state = p
				}
			}
			v[t][y] = best
			prev[t][y] = state
		}
	}
	path := make([]int, len(runes))
	last := len(runes) - 1
	state := 1
	if v[last][3] >= v[last][1] {
		state = 3
	}
	path[last] = state
	for t := last; t > 0; t-- {
		state = prev[t][state]
		path[t-1] = state
	}
	begin, next := 0, 0
	for i, state := range path {
		switch state {
		case 0:
			begin = pos[i]
		case 1:
			words = append(words, s[begin:pos[i+1]])
			next = pos[i+1]
		case 3:
			words = append(words, s[pos[i]:pos[i+1]])
			next = pos[i+1]
		}
	}
	if next < len(s) {
		words = append(words, s[next:])
	}
	return words
}
