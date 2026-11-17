package textcase

import "sort"

// IsAlphanumeric uses the same frozen Unicode edition as Lower.
func IsAlphanumeric(char rune) bool {
	index := sort.Search(len(alphanumericRanges), func(i int) bool { return alphanumericRanges[i][1] >= char })
	return index < len(alphanumericRanges) && alphanumericRanges[index][0] <= char
}
