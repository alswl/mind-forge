package cli

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
	"github.com/alswl/mind-forge/internal/term"
	"github.com/spf13/cobra"
	terminal "golang.org/x/term"
)

func termSelection(c *cobra.Command, a *app.Context) (term.Selection, error) {
	s := term.Selection{ExcludeTerms: termArray(c, "exclude-term"), ExcludeOriginals: termArray(c, "exclude-original"), IncludeSuggested: boolFlag(c, "include-suggested"), MinConfidence: termFloat(c, "min-confidence"), IncludeQuotes: boolFlag(c, "include-quotes")}
	for _, raw := range termArray(c, "term") {
		name, original, pair := strings.Cut(raw, ":")
		if name == "" || pair && original == "" {
			return s, output.Usage("invalid qualified term selector '"+raw+"'", "use --term NAME or --term NAME:ORIGINAL")
		}
		if pair {
			s.Pairs = append(s.Pairs, term.Pair{Term: name, Original: original})
		} else {
			s.Terms = append(s.Terms, name)
		}
	}
	rules := termArray(c, "ad-hoc")
	if path := optionalFlag(c, "ad-hoc-from"); path != nil {
		name := *path
		if !filepath.IsAbs(name) {
			name = filepath.Join(a.CWD, name)
		}
		raw, err := os.ReadFile(name)
		if err != nil {
			return s, err
		}
		for _, line := range strings.Split(string(raw), "\n") {
			line = strings.TrimSpace(line)
			if line != "" && !strings.HasPrefix(line, "#") {
				rules = append(rules, line)
			}
		}
	}
	for _, raw := range rules {
		original, correct, ok := strings.Cut(raw, "=>")
		if !ok {
			return s, output.Usage("invalid ad-hoc correction '"+raw+"': expected ORIGINAL=>CORRECT", "")
		}
		original, correct = strings.TrimSpace(original), strings.TrimSpace(correct)
		if original == "" || correct == "" {
			return s, output.Usage("invalid ad-hoc correction '"+raw+"': both sides must be non-empty", "")
		}
		s.AdHoc = append(s.AdHoc, model.Correction{Original: original, Correct: correct})
	}
	return s, nil
}

func emptyTermReport(fix, dry bool) model.TermLintReport {
	r := model.TermLintReport{Findings: []model.TermFinding{}, SkippedFiles: []string{}, ModifiedFiles: []string{}, Failures: []model.TermLintFailure{}}
	if fix && dry {
		zero := uint64(0)
		r.WouldFixCount = &zero
	}
	return r
}

func termLintReport(c *cobra.Command, a *app.Context, args []string, selection term.Selection, fix, dry bool) (model.TermLintReport, error) {
	empty := emptyTermReport(fix, dry)
	scope, err := termScope(c, a)
	if err != nil {
		return empty, err
	}
	terms, err := scope.Load(true)
	if err != nil {
		return empty, err
	}
	if scope.Kind == "project" {
		global, err := (term.Scope{Root: a.RepoRoot, Kind: "global"}).Load(true)
		if err != nil {
			return empty, err
		}
		terms = term.MergeTerms(terms, global)
	}
	if err := selection.Validate(terms); err != nil {
		return empty, output.Usage(err.Error(), "")
	}
	if scope.Kind == "global" && len(terms) == 0 {
		return empty, nil
	}
	base, target := scope.Root, scope.Root
	if len(args) > 0 {
		target = args[0]
		if !filepath.IsAbs(target) {
			anchor := scope.Root
			if scope.Kind == "global" {
				for dir := a.CWD; ; dir = filepath.Dir(dir) {
					if _, err := os.Stat(filepath.Join(dir, "mind.yaml")); err == nil {
						anchor = dir
						break
					}
					if filepath.Dir(dir) == dir {
						break
					}
				}
			}
			target = filepath.Join(anchor, target)
		}
		if _, err := os.Stat(target); err != nil {
			return empty, output.Usage(fmt.Sprintf("file not found\n  input:    %s\n  resolved: %s", args[0], target), "")
		}
		if scope.Kind == "project" {
			target, err = store.CanonicalWithin(scope.Root, target)
			if err != nil {
				return empty, err
			}
		} else if info, err := os.Stat(target); err == nil && info.IsDir() {
			canonical, err := filepath.EvalSymlinks(target)
			if err != nil {
				return empty, err
			}
			target = canonical
			if _, err := store.CanonicalWithin(a.RepoRoot, target); err != nil {
				base = target
			}
		}
	} else if scope.Kind == "project" {
		config, err := content.ConfigFor(scope.Root)
		if err != nil {
			return empty, err
		}
		layout, _ := app.ResolveEffectiveLayout(config)
		target = filepath.Join(scope.Root, layout.Articles)
		if _, err := os.Stat(target); os.IsNotExist(err) {
			return empty, nil
		}
	}
	return term.LintFiles(base, target, terms, term.LintOptions{Selection: selection, Fix: fix, DryRun: dry})
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
	data["summary"] = map[string]any{"errors": 0, "warnings": len(r.Findings), "info": 0, "fixed": r.FixedCount}
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
	if flagPresent(c, "max-warnings") && int64(len(r.Findings)) > maximum {
		code = 1
	}
	if code != 0 {
		return &commandExit{code: code}
	}
	return nil
}

func formatTermLint(r model.TermLintReport, fix, dry bool, targetType, pathHint string) string {
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
	if !fix && len(r.Findings) == 0 {
		if r.ScannedFiles == 0 && len(r.SkippedFiles) == 0 && len(r.Failures) == 0 {
			if targetType == "directory" {
				return "No eligible files found."
			}
			return "No terms registered."
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
