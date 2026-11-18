package textcase

import (
	"sort"
	"strings"
)

type lowerMapping struct {
	code rune
	text string
}
type runeRange struct{ first, last rune }

func containsRune(ranges []runeRange, char rune) bool {
	index := sort.Search(len(ranges), func(index int) bool { return ranges[index].last >= char })
	return index < len(ranges) && ranges[index].first <= char
}

func Lower(text string) string {
	runes := []rune(text)
	var result strings.Builder
	result.Grow(len(text))
	casedAt := func(start, step int) bool {
		for index := start; index >= 0 && index < len(runes); index += step {
			if containsRune(legacyIgnoredRanges, runes[index]) {
				continue
			}
			return containsRune(legacyCasedRanges, runes[index])
		}
		return false
	}
	for index, char := range runes {
		if char == 'Σ' && casedAt(index-1, -1) && !casedAt(index+1, 1) {
			result.WriteRune('ς')
			continue
		}
		position := sort.Search(len(legacyLowerMappings), func(index int) bool { return legacyLowerMappings[index].code >= char })
		if position < len(legacyLowerMappings) && legacyLowerMappings[position].code == char {
			result.WriteString(legacyLowerMappings[position].text)
		} else {
			result.WriteRune(char)
		}
	}
	return result.String()
}
