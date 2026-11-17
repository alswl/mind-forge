package term

import (
	"bytes"
	"strings"
	"testing"
)

func TestFrontMatterFrozenAssertions(t *testing.T) {
	for _, test := range []struct{ name, content, want string }{
		{"fm_none_when_no_front_matter", "Just text\nno front matter\n", "none"},
		{"fm_present_no_skip", "---\ntitle: test\n---\nbody text", "present"},
		{"fm_skip_mf_term_lint", "---\nmf_term_lint: skip\n---\nbody text", "skip"},
		{"fm_skip_mf_dash_term_lint", "---\nmf-term-lint: skip\n---\nbody text", "skip"},
		{"fm_crlf_handling", "---\r\nmf_term_lint: skip\r\n---\r\nbody", "skip"},
		{"fm_skip_leading_spaces", "---\n  mf_term_lint: skip\n---\nbody", "skip"},
	} {
		t.Run(test.name, func(t *testing.T) {
			end, skip := frontMatter(test.content)
			kind := "none"
			if skip {
				kind = "skip"
			} else if end > 0 {
				kind = "present"
			}
			if kind != test.want {
				t.Fatalf("end=%d skip=%t want=%s", end, skip, test.want)
			}
		})
	}
}

func TestExemptFrozenAssertions(t *testing.T) {
	mask := func(t *testing.T, input string, start int) []byte {
		t.Helper()
		out, _ := maskExempt(input, start, false)
		if len(out) != len(input) {
			t.Fatal("byte length changed")
		}
		return out
	}
	zero := func(t *testing.T, value []byte, newlines bool) {
		t.Helper()
		for _, b := range value {
			if b != 0 && !(newlines && b == '\n') {
				t.Fatalf("unmasked byte %d in %q", b, value)
			}
		}
	}
	t.Run("strip_plain_text_preserved", func(t *testing.T) {
		input := "hello world"
		if got := mask(t, input, 0); !bytes.Equal(got, []byte(input)) {
			t.Fatalf("%q", got)
		}
	})
	t.Run("strip_fenced_code_block_exempt", func(t *testing.T) {
		input := "before\n```\ncode mindrepo\n```\nafter"
		got := mask(t, input, 0)
		if string(got[:7]) != "before\n" || string(got[strings.LastIndex(input, "after"):]) != "after" {
			t.Fatalf("%q", got)
		}
		zero(t, got[11:11+len("code mindrepo")], false)
	})
	t.Run("strip_inline_code_exempt", func(t *testing.T) {
		input := "text `code mindrepo` more"
		got := mask(t, input, 0)
		zero(t, got[strings.Index(input, "`")+1:strings.LastIndex(input, "`")], false)
	})
	t.Run("strip_link_url_exempt", func(t *testing.T) {
		input := "[text](https://example.com/mindrepo)"
		got := mask(t, input, 0)
		zero(t, got[strings.Index(input, "(")+1:strings.Index(input, ")")], false)
	})
	t.Run("strip_bare_url_exempt", func(t *testing.T) {
		input := "visit https://example.com/mindrepo for info"
		got := mask(t, input, 0)
		start := strings.Index(input, "https://")
		end := start + strings.Index(input[start:], " ")
		zero(t, got[start:end], false)
	})
	t.Run("strip_front_matter_exempt", func(t *testing.T) {
		input := "---\ntitle: test\n---\nbody mindrepo"
		end, skip := frontMatter(input)
		if end <= 0 || skip {
			t.Fatal("missing front matter")
		}
		got := mask(t, input, end)
		zero(t, got[:end], false)
		if string(got[strings.Index(input, "body"):]) != "body mindrepo" {
			t.Fatalf("%q", got)
		}
	})
	t.Run("strip_block_exempt_markers", func(t *testing.T) {
		input := "before\n<!-- mf-term-lint:off -->\nsecret mindrepo\n<!-- mf-term-lint:on -->\nafter"
		got := mask(t, input, 0)
		if string(got[:7]) != "before\n" || string(got[strings.LastIndex(input, "after"):]) != "after" {
			t.Fatalf("%q", got)
		}
		start := strings.Index(input, "<!-- mf-term-lint:off -->") + len("<!-- mf-term-lint:off -->\n")
		end := strings.Index(input, "<!-- mf-term-lint:on -->")
		zero(t, got[start:end], true)
	})
	t.Run("strip_output_len_equals_input_len", func(t *testing.T) {
		for _, input := range []string{"plain text", "before\n```\ncode\n```\nafter", "text `inline` more", "a <!-- comment --> b", "[link](url) text", "visit http://example.com", "---\nkey: val\n---\nbody", "before\n<!-- mf-term-lint:off -->\nhidden\n<!-- mf-term-lint:on -->\nafter"} {
			end, _ := frontMatter(input)
			mask(t, input, end)
		}
	})
	t.Run("fenced_code_prevents_match", func(t *testing.T) {
		input := "text mindrepo\n```\nmindrepo in code\n```\nmindrepo after"
		got := mask(t, input, 0)
		zero(t, got[strings.Index(input, "```")+4:strings.LastIndex(input, "```")], true)
	})
	t.Run("tmux_fence_works", func(t *testing.T) {
		input := "text\n~~~\nfenced mindrepo\n~~~\nmore"
		got := mask(t, input, 0)
		zero(t, got[strings.Index(input, "~~~")+4:strings.LastIndex(input, "~~~")], true)
	})
	t.Run("blockquotes_and_cjk_quotes_are_exempt_by_default", func(t *testing.T) {
		input := "> quoted mindrepo\n>> nested mindrepo\nplain mindrepo 「verbatim mindrepo"
		got := mask(t, input, 0)
		plain := strings.Index(input, "plain")
		zero(t, got[:plain], true)
		if !bytes.Contains(got[plain:], []byte("m")) {
			t.Fatal("plain body was masked")
		}
		zero(t, got[strings.Index(input, "「"):], false)
	})
	t.Run("include_quotes_restores_scannable_text", func(t *testing.T) {
		input := "> quoted mindrepo 「verbatim mindrepo」"
		got, _ := maskExempt(input, 0, true)
		if !bytes.Equal(got, []byte(input)) {
			t.Fatalf("%q", got)
		}
	})
	for _, test := range []struct {
		name, input, want string
		unclosed          bool
	}{
		{"collecting_quote_spans_reports_blockquote_line_span", "plain\n> quoted 装置\nmore plain\n", "> quoted 装置", false},
		{"collecting_quote_spans_reports_cjk_quote_span_including_markers", "plain 「装置」 more", "「装置」", false},
		{"collecting_quote_spans_closes_unclosed_cjk_quote_at_end_of_content", "before 「unterminated 装置", "「unterminated 装置", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, spans := maskExempt(test.input, 0, false)
			if len(spans) != 1 {
				t.Fatal(spans)
			}
			span := spans[0]
			if test.input[span.start:span.end] != test.want {
				t.Fatal(spans)
			}
			if test.unclosed {
				if span.end != len(test.input) {
					t.Fatal(spans)
				}
				zero(t, got[span.start:span.end], false)
			}
		})
	}
	t.Run("collecting_quote_spans_is_empty_for_plain_text", func(t *testing.T) {
		_, spans := maskExempt("no quotes here at all\n", 0, false)
		if len(spans) != 0 {
			t.Fatal(spans)
		}
	})
	t.Run("collecting_quote_spans_ignores_code_fence_lookalikes", func(t *testing.T) {
		_, spans := maskExempt("```\n> not a quote, it's code\n「not a quote either」\n```\n", 0, false)
		if len(spans) != 0 {
			t.Fatal(spans)
		}
	})
	t.Run("collecting_quote_spans_matches_masked_bytes_exactly", func(t *testing.T) {
		input := "Before.\n> quoted 装置\n「verbatim 装置」\nAfter.\n"
		got, spans := maskExempt(input, 0, false)
		for _, span := range spans {
			zero(t, got[span.start:span.end], true)
		}
	})
}
