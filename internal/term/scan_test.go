package term

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func termRule(name, original, correct, kind, boundary string) model.Term {
	return model.Term{Term: name, Corrections: []model.Correction{{Original: original, Correct: correct, Match: kind, Boundary: boundary}}}
}

func TestShortCJKExactTokenAndExplicitAdvisory(t *testing.T) {
	terms := []model.Term{termRule("装置", "机器", "装置", "word", "standalone"), termRule("可以", "以可", "可以", "word", "standalone")}
	r, err := Scan("机器。以可独立验证和回退的方案。", "docs/a.md", terms, Selection{})
	if err != nil {
		t.Fatal(err)
	}
	if len(r.Findings) != 1 || r.Findings[0].Original != "机器" || r.Findings[0].Selection != "ambiguous" || !r.Findings[0].HeldBack || len(r.Fixes) != 0 {
		t.Fatalf("advisory/exact-token policy: %+v", r)
	}
	r, err = Scan("机器。", "docs/a.md", terms, Selection{Terms: []string{"装置"}})
	if err != nil || len(r.Fixes) != 1 || Apply("机器。", r.Fixes) != "装置。" {
		t.Fatalf("opt-in: %+v %v", r, err)
	}
}

func TestMixedAndLongCJKUseAlignedEdges(t *testing.T) {
	terms := []model.Term{termRule("网关API", "网关api", "网关API", "word", "standalone"), termRule("不可跨词", "小文", "替换", "word", "standalone")}
	r, err := Scan("网关api 缩小文件", "a.md", terms, Selection{})
	if err != nil || len(r.Findings) != 1 || r.Findings[0].Original != "网关api" || len(r.Fixes) != 1 {
		t.Fatalf("mixed/edge policy: %+v %v", r, err)
	}
}

func TestASCIIStandaloneAndLoose(t *testing.T) {
	input := "api xapi api_x api-x path/api api.md (api)"
	r, err := Scan(input, "a.md", []model.Term{termRule("API", "api", "API", "word", "standalone")}, Selection{})
	if err != nil || len(r.Findings) != 2 {
		t.Fatalf("standalone: %+v %v", r, err)
	}
	r, err = Scan(input, "a.md", []model.Term{termRule("API", "api", "API", "word", "loose")}, Selection{})
	if err != nil || len(r.Findings) != 5 {
		t.Fatalf("loose: %+v %v", r, err)
	}
}

func TestExemptRegionsAndQuoteDisclosure(t *testing.T) {
	input := "---\ntitle: api\n---\napi `api` [api](https://example.invalid/api) https://example.invalid/api\n```api\napi\n```\n<!-- api -->\n<!-- mf-term-lint:off -->\napi\n<!-- mf-term-lint:on -->\n> api\n「api」\napi\n"
	terms := []model.Term{termRule("API", "api", "API", "word", "standalone")}
	r, err := Scan(input, "a.md", terms, Selection{Terms: []string{"API"}})
	if err != nil {
		t.Fatal(err)
	}
	if len(r.Findings) != 5 || len(r.Fixes) != 3 {
		t.Fatalf("exemption/disclosure counts: %+v", r)
	}
	protected := 0
	for _, f := range r.Findings {
		if f.QuoteProtected {
			protected++
			if f.Selection != "quote_protected" || f.HeldBack || f.ReplacementEligible {
				t.Fatalf("quote override leaked: %+v", f)
			}
		}
	}
	if protected != 2 {
		t.Fatalf("protected %d", protected)
	}
	fixed := Apply(input, r.Fixes)
	if !strings.Contains(fixed, "> api\n「api」") || !strings.Contains(fixed, "`api`") || !strings.Contains(fixed, "[API](https://example.invalid/api)") {
		t.Fatalf("source protection: %s", fixed)
	}
	r, err = Scan(input, "a.md", terms, Selection{IncludeQuotes: true})
	if err != nil || len(r.Fixes) != 5 {
		t.Fatalf("include-quotes: %+v %v", r, err)
	}
}

func TestNoWildcardAcrossExemptUTF8(t *testing.T) {
	r, err := Scan("以`机器`及文件", "a.md", []model.Term{termRule("替换", "以可", "替换", "substring", "loose")}, Selection{})
	if err != nil || len(r.Findings) != 0 {
		t.Fatalf("masked bytes matched: %+v %v", r, err)
	}
}

func TestPinyinAlwaysSuggestedAndOnlyCJKRuns(t *testing.T) {
	terms := []model.Term{termRule("时刻", "时刻", "时刻", "pinyin", "standalone")}
	r, err := Scan("实刻 时刻 实A刻 `实刻`", "a.md", terms, Selection{})
	if err != nil || len(r.Findings) != 1 || r.Findings[0].Original != "实刻" || r.Findings[0].FixKind != "suggested" || r.Findings[0].Selection != "suggested_disabled" || len(r.Fixes) != 0 {
		t.Fatalf("pinyin: %+v %v", r, err)
	}
	r, err = Scan("实刻", "a.md", terms, Selection{IncludeSuggested: true})
	if err != nil || Apply("实刻", r.Fixes) != "时刻" {
		t.Fatalf("suggested opt-in: %+v %v", r, err)
	}
}

func TestLongestMatchCompetingTermsAndUTF8Columns(t *testing.T) {
	terms := []model.Term{termRule("One", "abc", "X", "substring", "loose"), termRule("Two", "abcdef", "Y", "substring", "loose"), termRule("Three", "abcdef", "Z", "substring", "loose")}
	r, err := Scan("机😀abcdef", "a.md", terms, Selection{})
	if err != nil || len(r.Findings) != 1 || r.Findings[0].Term != "Two" || r.Findings[0].Column != 3 || len(r.Findings[0].CompetingTerms) != 2 || Apply("机😀abcdef", r.Fixes) != "机😀Y" {
		t.Fatalf("ordering: %+v %v", r, err)
	}
}

func TestSelectionAndConfidence(t *testing.T) {
	confidence := 0.4
	a := termRule("API", "api", "API", "word", "standalone")
	a.Confidence = &confidence
	a.Corrections[0].Fix = "suggested"
	minimum := 0.5
	r, err := Scan("api", "a.md", []model.Term{a}, Selection{IncludeSuggested: true, MinConfidence: &minimum})
	if err != nil || r.Findings[0].Selection != "below_confidence" || len(r.Fixes) != 0 {
		t.Fatalf("confidence: %+v %v", r, err)
	}
	if _, err = Scan("api", "a.md", []model.Term{a}, Selection{MinConfidence: &minimum}); err == nil {
		t.Fatal("min confidence without suggested accepted")
	}
	if _, err = Scan("api", "a.md", []model.Term{a}, Selection{Terms: []string{"missing"}}); err == nil {
		t.Fatal("unknown term silently accepted")
	}
	r, err = Scan("api", "a.md", []model.Term{a}, Selection{ExcludeOriginals: []string{"api"}, IncludeSuggested: true})
	if err != nil || r.Findings[0].Selection != "excluded_original" {
		t.Fatalf("exclusion: %+v %v", r, err)
	}
}

func TestFrontMatterSkipAndPadding(t *testing.T) {
	input := "---\r\nmf-term-lint: skip\r\n---\r\napi\n"
	r, err := Scan(input, "a.md", []model.Term{termRule("API", "api", "API", "word", "")}, Selection{})
	if err != nil || !r.Skipped || len(r.Findings) != 0 {
		t.Fatalf("frontmatter skip: %+v %v", r, err)
	}
	r, err = Scan("机器api装置", "a.md", []model.Term{termRule("API", "api", "API", "word", "standalone")}, Selection{})
	if err != nil || Apply("机器api装置", r.Fixes) != "机器 API 装置" {
		t.Fatalf("outer spacing: %+v %v", r, err)
	}
}

func TestExplicitFileKeepsReferenceFrontMatterSemantics(t *testing.T) {
	input := "---\nmf-term-lint: skip\n---\napi\n"
	r, err := ScanFile(input, "a.md", []model.Term{termRule("API", "api", "API", "word", "standalone")}, Selection{})
	if err != nil || r.Skipped || len(r.Findings) != 1 {
		t.Fatalf("explicit file should still scan: %+v %v", r, err)
	}
}

func TestQuotedShortCJKCannotBeForcedByTermOrPair(t *testing.T) {
	terms := []model.Term{termRule("装置", "机器", "装置", "word", "standalone")}
	for _, selection := range []Selection{{Terms: []string{"装置"}}, {Pairs: []Pair{{Term: "装置", Original: "机器"}}}} {
		r, err := Scan("「机器」", "a.md", terms, selection)
		if err != nil || len(r.Findings) != 1 || len(r.Fixes) != 0 || r.Findings[0].Selection != "quote_protected" || r.Findings[0].HeldBack {
			t.Fatalf("quoted advisory promoted: %+v %v", r, err)
		}
	}
}

func TestLooseSubstringWarnsWithoutAddingPadding(t *testing.T) {
	r, err := Scan("xapi装置", "a.md", []model.Term{termRule("API", "api", "API", "substring", "loose")}, Selection{})
	if err != nil || len(r.Findings) != 1 || !r.Findings[0].SubstringAdjacentWord || Apply("xapi装置", r.Fixes) != "xAPI装置" {
		t.Fatalf("loose substring policy: %+v %v", r, err)
	}
}
