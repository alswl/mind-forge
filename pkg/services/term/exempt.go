package term

import (
	"bytes"
	"strings"
)

func frontMatter(content string) (end int, skip bool) {
	start := 0
	if strings.HasPrefix(content, "---\r\n") {
		start = 5
	} else if strings.HasPrefix(content, "---\n") {
		start = 4
	} else {
		return 0, false
	}
	for i := start; i+3 < len(content); i++ {
		if (i == start || content[i-1] == '\n') && strings.HasPrefix(content[i:], "---") {
			for _, line := range strings.Split(content[start:i], "\n") {
				line = strings.TrimSpace(line)
				if line == "mf_term_lint: skip" || line == "mf-term-lint: skip" {
					return 0, true
				}
			}
			end = i + 4
			if content[i+3] == '\r' {
				end++
			}
			return min(end, len(content)), false
		}
	}
	return 0, false
}

type byteRange struct{ start, end int }

// maskExempt preserves byte offsets. Quote spans are disclosed separately from
// code, URLs and comments so they can be reported without becoming fixable.
func maskExempt(content string, start int, includeQuotes bool) ([]byte, []byteRange) {
	const (
		body = iota
		fenceBacktick
		fenceTilde
		inline
		comment
		link
		url
		off
		quote
		cjkQuote
	)
	input := []byte(content)
	out := make([]byte, len(input))
	spans := []byteRange{}
	state := body
	quoteStart := -1
	lineStart := func(i int) bool { return i == start || i > 0 && input[i-1] == '\n' }
	repeat := func(i int, b byte) int {
		n := 0
		for i+n < len(input) && input[i+n] == b {
			n++
		}
		return n
	}
	commentEnd := func(i int) int {
		n := bytes.Index(input[i:], []byte("-->"))
		if n < 0 {
			return -1
		}
		return i + n
	}
	for i := start; i < len(input); {
		b := input[i]
		switch state {
		case body:
			cursor := i
			if !includeQuotes && lineStart(i) {
				for cursor < len(input) && cursor-i < 3 && input[cursor] == ' ' {
					cursor++
				}
				if cursor < len(input) && input[cursor] == '>' {
					quoteStart = i
					state = quote
					continue
				}
			}
			if !includeQuotes && bytes.HasPrefix(input[i:], []byte("「")) {
				quoteStart = i
				state = cjkQuote
				i += len("「")
				continue
			}
			if bytes.HasPrefix(input[i:], []byte("<!--")) {
				end := commentEnd(i + 4)
				if lineStart(i) && end >= 0 && strings.TrimSpace(content[i+4:end]) == "mf-term-lint:off" {
					copy(out[i:end+3], input[i:end+3])
					i = end + 3
					state = off
					continue
				}
				state = comment
				out[i] = b
				i++
				continue
			}
			if (b == '`' || b == '~') && lineStart(i) {
				n := repeat(i, b)
				if n >= 3 {
					state = fenceBacktick
					if b == '~' {
						state = fenceTilde
					}
					copy(out[i:i+n], input[i:i+n])
					i += n
					continue
				}
			}
			if b == '`' {
				state = inline
				out[i] = b
				i++
				continue
			}
			if bytes.HasPrefix(input[i:], []byte("](")) {
				state = link
				out[i] = b
				i++
				continue
			}
			if i+len("http://") < len(input) && bytes.HasPrefix(input[i:], []byte("http://")) || i+len("https://") < len(input) && bytes.HasPrefix(input[i:], []byte("https://")) {
				state = url
				i++
				continue
			}
			out[i] = b
			i++
		case fenceBacktick, fenceTilde:
			mark := byte('`')
			if state == fenceTilde {
				mark = '~'
			}
			if i == start || b == '\n' {
				check := i
				if b == '\n' {
					check++
				}
				if check < len(input) && input[check] == mark {
					n := repeat(check, mark)
					if n >= 3 {
						copy(out[check:check+n], input[check:check+n])
						i = check + n
						state = body
						continue
					}
				}
			}
			if b == '\n' || b == '\r' {
				out[i] = b
			}
			i++
		case inline:
			if b == '`' || b == '\n' {
				out[i] = b
				state = body
			}
			i++
		case comment:
			if bytes.HasPrefix(input[i:], []byte("-->")) {
				copy(out[i:i+3], input[i:i+3])
				i += 3
				state = body
				continue
			}
			i++
		case link:
			if b == ')' || b == '\n' {
				out[i] = b
				state = body
			}
			i++
		case url:
			if b == ' ' || b == '\t' || b == '\n' || b == '\r' || b == '>' || b == ')' {
				out[i] = b
				state = body
			}
			i++
		case off:
			if bytes.HasPrefix(input[i:], []byte("<!--")) && lineStart(i) {
				end := commentEnd(i + 4)
				if end >= 0 && strings.TrimSpace(content[i+4:end]) == "mf-term-lint:on" {
					copy(out[i:end+3], input[i:end+3])
					i = end + 3
					state = body
					continue
				}
			}
			if b == '\n' || b == '\r' {
				out[i] = b
			}
			i++
		case quote:
			if b == '\n' {
				out[i] = b
				state = body
				spans = append(spans, byteRange{quoteStart, i})
				quoteStart = -1
			} else if b == '\r' {
				out[i] = b
			}
			i++
		case cjkQuote:
			if bytes.HasPrefix(input[i:], []byte("」")) {
				i += len("」")
				state = body
				spans = append(spans, byteRange{quoteStart, i})
				quoteStart = -1
				continue
			}
			if b == '\n' || b == '\r' {
				out[i] = b
			}
			i++
		}
	}
	if quoteStart >= 0 {
		spans = append(spans, byteRange{quoteStart, len(input)})
	}
	return out, spans
}
