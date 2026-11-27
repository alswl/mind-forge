package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
	terminal "golang.org/x/term"
)

func termSelection(c *cobra.Command, a *app.Context) (term.Selection, error) {
	return term.BuildSelection(a.CWD, term.SelectionRequest{
		Selection: term.Selection{ExcludeTerms: termArray(c, "exclude-term"), ExcludeOriginals: termArray(c, "exclude-original"), IncludeSuggested: boolFlag(c, "include-suggested"), MinConfidence: termFloat(c, "min-confidence"), IncludeQuotes: boolFlag(c, "include-quotes")},
		Terms:     termArray(c, "term"), AdHoc: termArray(c, "ad-hoc"), AdHocFrom: optionalFlag(c, "ad-hoc-from"),
	})
}

func termLintReport(c *cobra.Command, a *app.Context, args []string, selection term.Selection, fix, dry bool) (model.TermLintReport, error) {
	scope, err := termScope(c, a)
	if err != nil {
		return model.TermLintReport{}, err
	}
	return term.LintRequest(c.Context(), a.RepoRoot, a.CWD, scope, args, selection, fix, dry)
}

var termLintRules = map[string]struct{}{
	"term_word": {}, "term_substring": {}, "term_pinyin": {},
}

func filterTermLintFindings(findings []model.TermFinding, rules []string, severity string) ([]model.TermFinding, error) {
	selectedRules := make(map[string]struct{}, len(rules))
	for _, rule := range rules {
		if _, ok := termLintRules[rule]; !ok {
			return nil, output.Usage(fmt.Sprintf("unknown term lint rule %q", rule), "valid rules: term_word, term_substring, term_pinyin")
		}
		selectedRules[rule] = struct{}{}
	}
	if severity != "" && severity != "error" && severity != "warning" && severity != "info" {
		return nil, output.Usage(fmt.Sprintf("invalid severity %q", severity), "valid severities: error, warning, info")
	}
	threshold := -1
	if severity != "" {
		threshold = severityRank(severity)
	}
	filtered := make([]model.TermFinding, 0, len(findings))
	for _, finding := range findings {
		if len(selectedRules) > 0 {
			if _, ok := selectedRules[finding.Rule]; !ok {
				continue
			}
		}
		if threshold >= 0 {
			if finding.Severity == "" || severityRank(finding.Severity) > threshold {
				continue
			}
		}
		filtered = append(filtered, finding)
	}
	return filtered, nil
}

func termLintCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	selection, err := termSelection(c, a)
	if err != nil {
		return err
	}
	fix := c.Name() == "fix" || boolFlag(c, "fix")
	dry := fix && boolFlag(c, "dry-run")
	if fix && !dry && !boolFlag(c, "yes") {
		file, isFile := stdout.(*os.File)
		if !isFile || !terminal.IsTerminal(int(file.Fd())) {
			return output.Usage("--fix in non-interactive context requires -y / --yes", "pass --yes to confirm")
		}
		if a.Format == "text" {
			preview, err := termLintReport(c, a, args, selection, true, true)
			if err != nil {
				return err
			}
			if len(preview.Findings) > 0 {
				if err := output.RenderRaw(stdout, "text", formatTermLint(preview, true, true, "", "")); err != nil {
					return err
				}
			}
		}
		switch output.PromptConfirmation("Apply changes? [y/N] ", stderr, nil) {
		case output.NotTTY:
			return output.Usage("--fix in non-interactive context requires -y / --yes", "pass --yes to confirm")
		case output.Aborted:
			return output.RenderRaw(stdout, "text", "Aborted by user.")
		}
	}
	targetType := "project"
	pathHint := ""
	if len(args) > 0 {
		pathHint = args[0]
		targetType = "file"
		for _, path := range []string{pathHint, filepath.Join(a.CWD, pathHint), filepath.Join(a.RepoRoot, pathHint)} {
			if info, err := os.Stat(path); err == nil && info.IsDir() {
				targetType = "directory"
				break
			}
		}
	}
	if flagPresent(c, "article") {
		targetType = "article"
	}
	r, err := termLintReport(c, a, args, selection, fix, dry)
	if err != nil {
		return err
	}
	severity := ""
	if flagPresent(c, "severity") {
		severity, _ = c.Flags().GetString("severity")
	}
	r.Findings, err = filterTermLintFindings(r.Findings, termArray(c, "rule"), severity)
	if err != nil {
		return err
	}
	warnings := []string{}
	for _, f := range r.Findings {
		if f.SubstringAdjacentWord {
			warnings = append(warnings, fmt.Sprintf("%s:%d:%d: substring match '%s' is adjacent to a continuous word character; it may be part of a larger word — consider registering the full variant", f.Path, f.Line, f.Column, f.Original))
		}
	}
	data, err := jsonObject(r)
	if err != nil {
		return err
	}
	data["issues"] = data["findings"]
	delete(data, "findings")
	data["kind"], data["dry_run"], data["target_type"], data["term_filter"] = "term", dry, targetType, termArray(c, "term")
	warningCount, infoCount := 0, 0
	for _, finding := range r.Findings {
		switch finding.Severity {
		case "error":
			// Term findings currently have no error classification.
		case "info":
			infoCount++
		default:
			warningCount++
		}
	}
	data["summary"] = map[string]any{"errors": 0, "warnings": warningCount, "info": infoCount, "fixed": r.FixedCount}
	text := formatTermLint(r, fix, dry, targetType, pathHint)
	if a.Format != "json" {
		for _, warning := range warnings {
			text += "\nwarning: " + warning
		}
		warnings = nil
	}
	if names := termArray(c, "term"); len(names) > 0 {
		text = "scoped to term(s): " + strings.Join(names, ", ") + "\n" + text
	}
	if err := renderTerm(a, stdout, stderr, data, text, warnings); err != nil {
		return err
	}
	code := 0
	if fix && !dry {
		if len(r.Failures) > 0 {
			code = 1
		}
	} else if len(r.Findings) > 0 {
		code = 1
	}
	maximum, _ := c.Flags().GetInt64("max-warnings")
	if flagPresent(c, "max-warnings") && int64(warningCount) > maximum {
		code = 1
	}
	if code != 0 {
		return &commandExit{code: code}
	}
	return nil
}

func formatTermLint(r model.TermLintReport, fix, dry bool, targetType, pathHint string) string {
	if r.NoTermsRegistered && len(r.Findings) == 0 && len(r.Failures) == 0 {
		return "No terms registered."
	}
	lines := []string{}
	if targetType != "" {
		mode := ""
		if fix && dry {
			mode = " (dry-run preview)"
		}
		lines = append(lines, "target: "+targetType+mode)
	}
	if fix && len(r.Findings) == 0 && len(r.Failures) == 0 {
		return "No term issues found."
	}
	if !fix && len(r.Findings) == 0 && r.SuppressedHeldBackCount == 0 {
		if r.ScannedFiles == 0 && len(r.SkippedFiles) == 0 && len(r.Failures) == 0 {
			if targetType == "directory" {
				return "No eligible files found."
			}
			return "No term issues found."
		}
		if len(r.Failures) == 0 {
			return "No term issues found."
		}
	}
	for _, f := range r.Findings {
		if fix && !dry {
			continue
		}
		if dry && fix {
			confidence := "none"
			if f.Confidence != nil {
				confidence = fmt.Sprintf("%.2f", *f.Confidence)
			}
			lines = append(lines, fmt.Sprintf("%s:%d:%d: \"%s\" → \"%s\" [%s] match=%s confidence=%s selection=%s", f.Path, f.Line, f.Column, f.Original, f.Correct, f.Term, f.MatchKind, confidence, f.Selection), "  "+f.Context)
			continue
		}
		marks := ""
		if f.Confidence != nil {
			marks = fmt.Sprintf(" [confidence=%.2f]", *f.Confidence)
		}
		if f.FixKind == "suggested" {
			marks += "?"
		}
		if f.Boundary == "standalone" {
			marks += ", standalone"
		}
		if f.HeldBack {
			marks += ", held back"
		}
		if f.QuoteProtected {
			marks += ", quote-protected"
		}
		lines = append(lines, fmt.Sprintf("%s:%d:%d: \"%s\" → \"%s\" [%s]%s", f.Path, f.Line, f.Column, f.Original, f.Correct, f.Term, marks))
		if len(f.CompetingTerms) > 0 {
			lines = append(lines, "  also claimed by: "+strings.Join(f.CompetingTerms, ", "))
		}
	}
	if fix && !dry {
		for _, path := range r.ModifiedFiles {
			count := 0
			for _, f := range r.Findings {
				if f.Path == path && f.Selection == "selected" {
					count++
				}
			}
			suffix := "s"
			if count == 1 {
				suffix = ""
			}
			lines = append(lines, fmt.Sprintf("✓ fixed: %s (%d replacement%s)", path, count, suffix))
		}
		for _, f := range r.Failures {
			lines = append(lines, "✗ failed: "+f.Path+" — "+f.Reason)
		}
	}
	paths := map[string]bool{}
	for _, f := range r.Findings {
		paths[f.Path] = true
	}
	plural := "s"
	if len(r.Failures) == 1 {
		plural = ""
	}
	if fix && dry {
		count := uint64(0)
		if r.WouldFixCount != nil {
			count = *r.WouldFixCount
		}
		lines = append(lines, fmt.Sprintf("%d findings in %d files (selected %d, excluded %d, below confidence %d, ineligible %d, %d failure%s)", len(r.Findings), len(paths), count, r.ExcludedCount, r.BelowConfidenceCount, r.IneligibleCount, len(r.Failures), plural))
	} else if fix {
		lines = append(lines, fmt.Sprintf("%d findings in %d files (%d fixed, %d failure%s)", len(r.Findings), len(paths), r.FixedCount, len(r.Failures), plural))
		if r.HeldBackCount > 0 {
			name := "TERM"
			for _, f := range r.Findings {
				if f.HeldBack {
					name = f.Term
					break
				}
			}
			path := pathHint
			if path == "" {
				path = "<PATH>"
			}
			s := "s"
			if r.HeldBackCount == 1 {
				s = ""
			}
			lines = append(lines, fmt.Sprintf("%d finding%s held back; apply with `mf term fix %s --term %s`", r.HeldBackCount, s, path, name))
		}
	} else {
		lines = append(lines, fmt.Sprintf("%d findings in %d files", len(r.Findings), len(paths)))
		if r.SuppressedHeldBackCount > 0 {
			lines = append(lines, fmt.Sprintf("Held-back findings: %d (%d suppressed by project configuration)", r.HeldBackCount, r.SuppressedHeldBackCount))
		}
	}
	if r.QuoteProtectedCount > 0 {
		s := "s"
		if r.QuoteProtectedCount == 1 {
			s = ""
		}
		lines = append(lines, fmt.Sprintf("%d finding%s inside quotes/blockquotes left untouched; pass --include-quotes to scan them", r.QuoteProtectedCount, s))
	}
	return strings.Join(lines, "\n")
}
