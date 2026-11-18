package main

import (
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestFilterTermLintFindings(t *testing.T) {
	findings := []model.TermFinding{
		{Rule: "term_word", Severity: "warning", Original: "机器"},
		{Rule: "term_substring", Severity: "warning", Original: "alpha"},
		{Rule: "term_pinyin", Severity: "info", Original: "jiqi"},
	}

	got, err := filterTermLintFindings(findings, []string{"term_word", "term_pinyin"}, "warning")
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got[0].Original != "机器" {
		t.Fatalf("filtered findings = %#v, want only term_word warning", got)
	}

	got, err = filterTermLintFindings(findings, nil, "error")
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 0 {
		t.Fatalf("error-only findings = %#v, want none", got)
	}

	if _, err := filterTermLintFindings(findings, []string{"unknown"}, ""); err == nil {
		t.Fatal("unknown rule accepted")
	}
	if _, err := filterTermLintFindings(findings, nil, "fatal"); err == nil {
		t.Fatal("unknown severity accepted")
	}
}
