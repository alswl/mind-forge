package text

import (
	"strconv"
	"strings"
	"sync"
)

var readings = sync.OnceValue(func() map[rune]string {
	data := make(map[rune]string, 41497)
	for _, line := range strings.Split(string(unpack(pinyinData)), "\n") {
		if line == "" {
			continue
		}
		cp, plain, ok := strings.Cut(line, "\t")
		if !ok {
			panic("invalid embedded pinyin row")
		}
		n, err := strconv.ParseUint(cp, 16, 32)
		if err != nil {
			panic(err)
		}
		data[rune(n)] = plain
	}
	return data
})

// Pinyin joins the frozen dictionary's first, toneless reading for each known
// scalar. Unknown scalars contribute neither text nor separators.
func Pinyin(input string) string {
	var result strings.Builder
	data := readings()
	for _, r := range input {
		if plain := data[r]; plain != "" {
			if result.Len() > 0 {
				result.WriteByte('-')
			}
			result.WriteString(plain)
		}
	}
	return result.String()
}
