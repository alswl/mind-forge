package term

import (
	"bytes"
	"slices"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	textutil "github.com/alswl/mind-forge/internal/text"
	"github.com/alswl/mind-forge/internal/textcase"
)

type Replacement struct {
	Start, End, Order int
	Text              string
}

type ScanResult struct {
	Findings []model.TermFinding
	Fixes    []Replacement
	Skipped  bool
}

type rule struct {
	correction model.Correction
	term       model.Term
	order      int
}

func isCJK(r rune) bool {
	return r >= 0x4e00 && r <= 0x9fff || r >= 0x3400 && r <= 0x4dbf || r >= 0x20000 && r <= 0x3134f || r >= 0x3040 && r <= 0x30ff || r >= 0xac00 && r <= 0xd7af
}

func shortCJK(s string) bool {
	n := 0
	for _, r := range s {
		if !isCJK(r) {
			return false
		}
		n++
		if n > 2 {
			return false
		}
	}
	return n > 0
}

func ascii(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= utf8.RuneSelf {
			return false
		}
	}
	return true
}
func alnum(r rune) bool      { return r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' }
func identifier(b byte) bool { return alnum(rune(b)) || strings.ContainsRune("_-/\\.", rune(b)) }

func wordPolicy(c model.Correction) string {
	if c.Match == "substring" {
		if c.Boundary == "loose" {
			return "loose"
		}
		return "standalone"
	}
	if !ascii(c.Original) {
		return "cjk"
	}
	if c.Boundary == "loose" {
		return "ascii-loose"
	}
	return "standalone"
}

func accepts(content string, mask []byte, c model.Correction, start, end int, segments textutil.Segmentation) bool {
	switch wordPolicy(c) {
	case "loose":
		return true
	case "ascii-loose":
		return (start == 0 || !alnum(rune(mask[start-1])) && mask[start-1] != '_') && (end == len(mask) || !alnum(rune(mask[end])) && mask[end] != '_')
	case "standalone":
		if ascii(content[start:end]) {
			return (start == 0 || !identifier(mask[start-1])) && (end == len(mask) || !identifier(mask[end]))
		}
		return segments.Aligned(start, end-start)
	default:
		if shortCJK(c.Original) {
			return segments.Exact(start, end-start)
		}
		return segments.Aligned(start, end-start)
	}
}

func adjacent(content string, start, end int) bool {
	before, after := rune(0), rune(0)
	if start > 0 {
		before, _ = utf8.DecodeLastRuneInString(content[:start])
	}
	if end < len(content) {
		after, _ = utf8.DecodeRuneInString(content[end:])
	}
	return isCJK(before) || alnum(before) || isCJK(after) || alnum(after)
}

func padded(content string, start, end int, replacement string) string {
	first, _ := utf8.DecodeRuneInString(replacement)
	last, _ := utf8.DecodeLastRuneInString(replacement)
	before, after := rune(0), rune(0)
	if start > 0 {
		before, _ = utf8.DecodeLastRuneInString(content[:start])
	}
	if end < len(content) {
		after, _ = utf8.DecodeRuneInString(content[end:])
	}
	if isCJK(before) && alnum(first) || alnum(before) && isCJK(first) {
		replacement = " " + replacement
	}
	if alnum(last) && isCJK(after) || isCJK(last) && alnum(after) {
		replacement += " "
	}
	return replacement
}

func excerpt(content string, start, end int) string {
	lineStart := strings.LastIndexByte(content[:start], '\n') + 1
	lineEnd := len(content)
	if n := strings.IndexByte(content[end:], '\n'); n >= 0 {
		lineEnd = end + n
	}
	line := strings.NewReplacer("\r", " ", "\t", " ").Replace(strings.TrimSpace(content[lineStart:lineEnd]))
	chars := []rune(line)
	if len(chars) <= 120 {
		return line
	}
	left := max(0, utf8.RuneCountInString(content[lineStart:start])-40)
	left = min(left, len(chars))
	right := min(left+120, len(chars))
	part := string(chars[left:right])
	if left > 0 {
		part = "…" + part
	}
	if right < len(chars) {
		part += "…"
	}
	return part
}

func prefixRelated(a, b string) bool {
	a = textcase.ASCIILower(a)
	b = textcase.ASCIILower(b)
	return strings.HasPrefix(a, b) || strings.HasPrefix(b, a)
}

// Scan computes findings and selected replacements for a single immutable
// document. File traversal and publication are owned by the calling service.
func Scan(content, path string, terms []model.Term, selection Selection) (ScanResult, error) {
	return scanDocument(content, path, terms, selection, true)
}

// ScanFile preserves the explicit-file path's treatment of frontmatter as
// ordinary input; directory scans use Scan's frontmatter skip directives.
func ScanFile(content, path string, terms []model.Term, selection Selection) (ScanResult, error) {
	return scanDocument(content, path, terms, selection, false)
}

func scanDocument(content, path string, terms []model.Term, selection Selection, inspectFrontMatter bool) (ScanResult, error) {
	result := ScanResult{Findings: []model.TermFinding{}, Fixes: []Replacement{}}
	if err := selection.Validate(terms); err != nil {
		return result, err
	}
	end, skip := 0, false
	if inspectFrontMatter {
		end, skip = frontMatter(content)
	}
	if skip {
		result.Skipped = true
		return result, nil
	}
	mask, quotes := maskExempt(content, end, selection.IncludeQuotes)
	segments := textutil.Segment(content)
	rules := []rule{}
	for _, term := range terms {
		for _, c := range term.Corrections {
			rules = append(rules, rule{c, term, len(rules)})
		}
	}
	for _, c := range selection.AdHoc {
		rules = append(rules, rule{c, model.Term{Term: "ad-hoc", Description: stringPtr("invocation-only correction")}, len(rules)})
	}
	ordered := append([]rule{}, rules...)
	sort.SliceStable(ordered, func(i, j int) bool { return len(ordered[i].correction.Original) > len(ordered[j].correction.Original) })
	claimed := []byteRange{}
	emit := func(r rule, start, end int, original string, phonetic, protected bool) {
		c := r.correction
		kind, fix, boundary := c.Match, c.Fix, c.Boundary
		if kind == "" {
			kind = "word"
		}
		if fix == "" {
			fix = "required"
		}
		if boundary == "" {
			boundary = "standalone"
		}
		mode := wordPolicy(c)
		if mode == "ascii-loose" {
			mode = "loose"
		}
		advisory := kind == "word" && shortCJK(c.Original)
		if phonetic {
			kind = "pinyin"
			fix = "suggested"
			boundary = "loose"
			mode = "loose"
			advisory = false
		}
		line := uint32(strings.Count(content[:start], "\n") + 1)
		lineStart := strings.LastIndexByte(content[:start], '\n') + 1
		f := model.TermFinding{Path: path, Line: line, Column: uint32(utf8.RuneCountInString(content[lineStart:start]) + 1), Original: original, Correct: c.Correct, Term: r.term.Term, Description: r.term.Description, Confidence: r.term.Confidence, ReplacementEligible: !advisory && !protected, MatchKind: kind, FixKind: fix, Boundary: boundary, BoundaryMode: mode, Context: excerpt(content, start, end), QuoteProtected: protected}
		if advisory {
			f.SafetyReason = stringPtr("short-cjk-advisory")
		}
		if protected {
			f.SafetyReason = stringPtr("quote-protected")
		}
		if !phonetic {
			for _, term := range terms {
				if term.Term != r.term.Term && prefixRelated(c.Original, term.Term) && !slices.Contains(f.CompetingTerms, term.Term) {
					f.CompetingTerms = append(f.CompetingTerms, term.Term)
				}
			}
			for _, other := range rules {
				if other.term.Term != r.term.Term && prefixRelated(c.Original, other.correction.Original) && !slices.Contains(f.CompetingTerms, other.term.Term) {
					f.CompetingTerms = append(f.CompetingTerms, other.term.Term)
				}
			}
			f.SubstringAdjacentWord = kind == "substring" && boundary == "loose" && adjacent(content, start, end)
		}
		f.Selection = selection.classify(f, advisory)
		f.HeldBack = f.Selection == "ambiguous" && advisory
		result.Findings = append(result.Findings, f)
		if f.Selection == "selected" {
			replacement := c.Correct
			if !f.SubstringAdjacentWord {
				replacement = padded(content, start, end, replacement)
			}
			result.Fixes = append(result.Fixes, Replacement{Start: start, End: end, Text: replacement, Order: r.order})
		}
		claimed = append(claimed, byteRange{start, end})
	}
	literal := func(mask []byte, protected bool) {
		for _, r := range ordered {
			c := r.correction
			if c.Match == "pinyin" || c.Original == "" {
				continue
			}
			for search := 0; search < len(mask); {
				n := bytes.Index(mask[search:], []byte(c.Original))
				if n < 0 {
					break
				}
				start := search + n
				end := start + len(c.Original)
				search = start + 1
				if slices.ContainsFunc(claimed, func(span byteRange) bool { return start < span.end && span.start < end }) || !accepts(content, mask, c, start, end, segments) {
					continue
				}
				emit(r, start, end, c.Original, false, protected)
			}
		}
	}
	literal(mask, false)
	// Phonetic matching uses contiguous unmasked CJK runs and declaration order.
	positions := []int{}
	runes := []rune{}
	flush := func() {
		for _, r := range rules {
			c := r.correction
			if c.Match != "pinyin" {
				continue
			}
			py := textutil.Pinyin(c.Original)
			if c.Pinyin != nil {
				py = *c.Pinyin
			}
			n := utf8.RuneCountInString(c.Original)
			if py == "" || n == 0 {
				continue
			}
			for i := 0; i+n <= len(runes); i++ {
				start := positions[i]
				if slices.ContainsFunc(claimed, func(span byteRange) bool { return span.start == start }) {
					continue
				}
				original := string(runes[i : i+n])
				if original == c.Original || textutil.Pinyin(original) != py {
					continue
				}
				emit(r, start, start+len(original), original, true, false)
			}
		}
		positions = nil
		runes = nil
	}
	for i, r := range content {
		if isCJK(r) && mask[i] != 0 {
			positions = append(positions, i)
			runes = append(runes, r)
		} else if len(runes) > 0 {
			flush()
		}
	}
	if len(runes) > 0 {
		flush()
	}
	if len(quotes) > 0 {
		quoteMask := make([]byte, len(content))
		for _, span := range quotes {
			copy(quoteMask[span.start:span.end], content[span.start:span.end])
		}
		literal(quoteMask, true)
	}
	return result, nil
}

func stringPtr(s string) *string { return &s }

func Apply(content string, spans []Replacement) string {
	spans = chosenSpans(content, spans)
	var out strings.Builder
	end := 0
	for _, span := range spans {
		out.WriteString(content[end:span.Start])
		out.WriteString(span.Text)
		end = span.End
	}
	out.WriteString(content[end:])
	return out.String()
}

func chosenSpans(content string, spans []Replacement) []Replacement {
	spans = append([]Replacement{}, spans...)
	sort.SliceStable(spans, func(i, j int) bool {
		a, b := spans[i], spans[j]
		if a.Start != b.Start {
			return a.Start < b.Start
		}
		if a.End != b.End {
			return a.End > b.End
		}
		return a.Order < b.Order
	})
	chosen := make([]Replacement, 0, len(spans))
	end := 0
	for _, span := range spans {
		if span.Start < end || span.Start < 0 || span.End < span.Start || span.End > len(content) {
			continue
		}
		chosen = append(chosen, span)
		end = span.End
	}
	return chosen
}
