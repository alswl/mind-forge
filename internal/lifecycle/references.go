package lifecycle

import (
	"fmt"
	"net/url"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/markdown"
	"github.com/alswl/mind-forge/internal/textcase"
)

func RelocatedPath(name string, moves map[string]*string) (string, bool) {
	mapped := Relocated(name, moves)
	if mapped == nil {
		return "", false
	}
	return *mapped, true
}

func referenceError(code, origin string, offset int, detail string) error {
	return rejected(code, fmt.Sprintf("%s at byte %d: %s", origin, offset, detail))
}

func referenceReplacement(origin, newOrigin, value string, offset int, moves map[string]*string) (string, bool, error) {
	fail := func(message string) (string, bool, error) {
		return "", false, referenceError("reference_unsupported", origin, offset, message)
	}
	if strings.HasPrefix(value, "/") && !strings.HasPrefix(value, "//") {
		return fail("site-root references have no verified repository base")
	}
	first := value
	if end := strings.IndexAny(value, "/#?"); end >= 0 {
		first = value[:end]
	}
	if value == "" || strings.HasPrefix(value, "#") || strings.HasPrefix(value, "//") || strings.Contains(first, ":") {
		return "", false, nil
	}
	if strings.ContainsAny(value, "{}$&") {
		return fail("dynamic or entity-encoded local reference")
	}
	split := strings.IndexAny(value, "#?")
	if split < 0 {
		split = len(value)
	}
	raw, suffix := value[:split], value[split:]
	// Backslash escapes and percent escapes are decoded in a single pass.
	var decoded strings.Builder
	for at := 0; at < len(raw); at++ {
		switch raw[at] {
		case '\\':
			at++
			if at >= len(raw) {
				return fail("invalid destination escaping")
			}
			decoded.WriteByte(raw[at])
		case '%':
			if at+2 >= len(raw) {
				return fail("invalid destination escaping")
			}
			part, err := url.PathUnescape(raw[at : at+3])
			if err != nil {
				return fail("invalid destination escaping")
			}
			decoded.WriteString(part)
			at += 2
		default:
			decoded.WriteByte(raw[at])
		}
	}
	if !utf8.ValidString(decoded.String()) {
		return fail("invalid destination escaping")
	}
	target := path.Clean(path.Join(path.Dir(origin), decoded.String()))
	if strings.HasPrefix(decoded.String(), "/") {
		target = path.Clean(decoded.String())
	}
	mapped, exists := RelocatedPath(target, moves)
	if !exists {
		return "", false, referenceError("inbound_reference", origin, offset, "references deleted "+target)
	}
	if mapped == target && origin == newOrigin {
		return "", false, nil
	}
	relative, err := filepath.Rel(path.Dir(newOrigin), mapped)
	if err != nil {
		return fail("cannot rebase destination")
	}
	relative = filepath.ToSlash(relative)
	if strings.HasSuffix(raw, "/") && !strings.HasSuffix(relative, "/") {
		relative += "/"
	}
	var encoded strings.Builder
	for _, char := range relative {
		if strings.ContainsRune(" ()#?%\"'<>", char) {
			fmt.Fprintf(&encoded, "%%%02X", char)
		} else {
			encoded.WriteRune(char)
		}
	}
	return encoded.String() + suffix, true, nil
}

func asciiSpace(b byte) bool {
	return b == ' ' || b == '\t' || b == '\n' || b == '\r' || b == '\v' || b == '\f'
}
func skipReferenceSpace(data []byte, at int) int {
	for at < len(data) && asciiSpace(data[at]) {
		at++
	}
	return at
}

type referenceRange struct{ start, end int }

func referenceDestination(data []byte, start int) (referenceRange, bool) {
	start = skipReferenceSpace(data, start)
	if start < len(data) && data[start] == '<' {
		if end := strings.IndexByte(string(data[start+1:]), '>'); end >= 0 {
			return referenceRange{start + 1, start + 1 + end}, true
		}
		return referenceRange{}, false
	}
	at, depth := start, 0
	for at < len(data) {
		switch data[at] {
		case '\\':
			at += 2
			continue
		case '(':
			depth++
		case ')':
			if depth == 0 {
				return referenceRange{start, at}, true
			}
			depth--
		default:
			if asciiSpace(data[at]) && depth == 0 {
				return referenceRange{start, at}, true
			}
		}
		at++
	}
	return referenceRange{start, at}, depth == 0 && at <= len(data)
}

func maskedReferences(text string) []byte {
	masked := []byte(text)
	mask := func(start, end int) {
		for i := start; i < end; i++ {
			masked[i] = ' '
		}
	}
	var fence markdown.FenceTracker
	offset := 0
	for _, line := range strings.SplitAfter(text, "\n") {
		logical := strings.TrimLeft(line, " ")
		for strings.HasPrefix(logical, ">") {
			logical = strings.TrimLeft(logical[1:], " ")
		}
		for _, marker := range []string{"- ", "* ", "+ "} {
			if strings.HasPrefix(logical, marker) {
				logical = logical[2:]
				break
			}
		}
		if fence.ProcessLine(logical) || strings.HasPrefix(line, "    ") || strings.HasPrefix(line, "\t") {
			mask(offset, offset+len(line))
		}
		offset += len(line)
	}
	for at := 0; at < len(masked); {
		escapes := 0
		for before := at - 1; before >= 0 && masked[before] == '\\'; before-- {
			escapes++
		}
		if masked[at] != '`' || escapes%2 == 1 {
			at++
			continue
		}
		start := at
		for at < len(masked) && masked[at] == '`' {
			at++
		}
		size := at - start
		for end := at; end < len(masked); {
			if masked[end] != '`' {
				end++
				continue
			}
			run := end
			for end < len(masked) && masked[end] == '`' {
				end++
			}
			if end-run == size {
				mask(start, end)
				at = end
				break
			}
		}
	}
	lower := textcase.ASCIILower(text)
	for _, pair := range [][2]string{{"<!--", "-->"}, {"<script", "</script>"}, {"<style", "</style>"}, {"<code", "</code>"}, {"<pre", "</pre>"}} {
		for offset := 0; offset < len(text); {
			start := strings.Index(lower[offset:], pair[0])
			if start < 0 {
				break
			}
			start += offset
			end := strings.Index(lower[start:], pair[1])
			if end < 0 {
				end = len(text)
			} else {
				end += start + len(pair[1])
			}
			mask(start, end)
			offset = end
		}
	}
	return masked
}

// RewriteAuthoredReferences changes only verified local destinations. Unsupported
// syntax is refused before any transaction writes author content.
func RewriteAuthoredReferences(text, origin, newOrigin string, moves map[string]*string) (string, error) {
	masked := maskedReferences(text)
	ranges := []referenceRange{}
	brackets := 0
	fail := func(at int, detail string) (string, error) {
		return "", referenceError("reference_unsupported", origin, at, detail)
	}
	atByte := func(at int, b byte) bool { return at >= 0 && at < len(masked) && masked[at] == b }
	for at := 0; at < len(masked); {
		rest := string(masked[at:])
		if strings.HasPrefix(rest, "[[") {
			return fail(at, "wiki links have no verified path semantics")
		}
		if masked[at] == '\\' {
			at += 2
			continue
		}
		if brackets > 0 && strings.HasPrefix(rest, "](") {
			r, ok := referenceDestination(masked, at+2)
			if !ok {
				return fail(at, "unbalanced Markdown destination")
			}
			tail := r.end
			if atByte(r.start-1, '<') {
				tail++
			}
			tail = skipReferenceSpace(masked, tail)
			if atByte(tail, '\'') || atByte(tail, '"') {
				quote := masked[tail]
				tail++
				for {
					if tail >= len(masked) {
						return fail(at, "unterminated link title")
					}
					if masked[tail] == '\\' {
						tail += 2
					} else if masked[tail] == quote {
						tail = skipReferenceSpace(masked, tail+1)
						break
					} else {
						tail++
					}
				}
			}
			if !atByte(tail, ')') {
				return fail(at, "malformed Markdown link")
			}
			at = tail + 1
			brackets--
			ranges = append(ranges, r)
			continue
		}
		lineStart := strings.LastIndexByte(string(masked[:at]), '\n') + 1
		if masked[at] == '[' && strings.Trim(string(masked[lineStart:at]), " ") == "" {
			end := strings.IndexAny(rest, "]\n")
			if end >= 0 && atByte(at+end, ']') && atByte(at+end+1, ':') {
				end += at + 2
				r, ok := referenceDestination(masked, end)
				if !ok {
					return fail(at, "malformed reference definition")
				}
				at = max(r.end, end)
				ranges = append(ranges, r)
				continue
			}
		}
		if strings.HasPrefix(rest, "]:") {
			r, ok := referenceDestination(masked, at+2)
			if !ok {
				return fail(at, "unrecognized reference definition")
			}
			_, changed, err := referenceReplacement(origin, newOrigin, text[r.start:r.end], at, moves)
			if err != nil {
				return "", err
			}
			if changed {
				return fail(at, "reference definition in unsupported container")
			}
		}
		if masked[at] == '<' && at+1 < len(masked) && asciiAlpha(masked[at+1]) {
			cursor, quote := at+1, byte(0)
			for cursor < len(masked) {
				b := masked[cursor]
				if quote != 0 {
					if b == quote {
						quote = 0
					}
				} else if b == '\'' || b == '"' {
					quote = b
				} else if b == '>' {
					break
				}
				cursor++
			}
			if cursor == len(masked) {
				return fail(at, "unterminated HTML tag")
			}
			for pos := at + 1; pos < cursor; {
				start := pos
				for pos < cursor && (asciiAlpha(masked[pos]) || masked[pos] >= '0' && masked[pos] <= '9' || masked[pos] == '-' || masked[pos] == ':') {
					pos++
				}
				if pos == start {
					pos++
					continue
				}
				attr := textcase.ASCIILower(string(masked[start:pos]))
				pos = skipReferenceSpace(masked, pos)
				if !atByte(pos, '=') {
					continue
				}
				pos = skipReferenceSpace(masked, pos+1)
				quote := byte(0)
				if atByte(pos, '\'') || atByte(pos, '"') {
					quote = masked[pos]
					pos++
				}
				valueStart := pos
				for pos < cursor && (quote == 0 && !asciiSpace(masked[pos]) || quote != 0 && masked[pos] != quote) {
					pos++
				}
				if attr == "src" || attr == "href" {
					ranges = append(ranges, referenceRange{valueStart, pos})
				} else if attr == "srcset" || attr == "style" || strings.HasSuffix(string(masked[start:start+len(attr)]), ":href") || strings.HasSuffix(string(masked[start:start+len(attr)]), ":src") {
					return fail(start, "srcset/CSS references require explicit maintenance")
				}
				if quote != 0 {
					pos++
				}
			}
			at = cursor + 1
			continue
		}
		if masked[at] == '[' {
			brackets++
		}
		if masked[at] == ']' && brackets > 0 {
			brackets--
		}
		at++
	}
	sort.Slice(ranges, func(i, j int) bool { return ranges[i].start < ranges[j].start })
	result := text
	last := referenceRange{-1, -1}
	for i := len(ranges) - 1; i >= 0; i-- {
		r := ranges[i]
		if r == last {
			continue
		}
		last = r
		value, changed, err := referenceReplacement(origin, newOrigin, text[r.start:r.end], r.start, moves)
		if err != nil {
			return "", err
		}
		if changed {
			result = result[:r.start] + value + result[r.end:]
		}
	}
	return result, nil
}

func asciiAlpha(b byte) bool { return b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' }
