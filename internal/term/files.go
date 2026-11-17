package term

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

type LintOptions struct {
	Selection   Selection
	Fix, DryRun bool
}

// LintFiles scans one selected file or one directory tree. Repository and
// article selection, global fallback and confirmation stay with the caller.
func LintFiles(base, target string, terms []model.Term, options LintOptions) (model.TermLintReport, error) {
	r := model.TermLintReport{Findings: []model.TermFinding{}, SkippedFiles: []string{}, ModifiedFiles: []string{}, Failures: []model.TermLintFailure{}}
	if options.Fix && options.DryRun {
		zero := uint64(0)
		r.WouldFixCount = &zero
	}
	if err := options.Selection.Validate(terms); err != nil {
		return r, output.Usage(err.Error(), "")
	}
	info, err := os.Stat(target)
	if err != nil {
		return r, output.Usage("file not found: "+target, "provide a path relative to the search root")
	}
	directory := info.IsDir()
	paths := []string{target}
	if directory {
		paths = nil
		_ = filepath.WalkDir(target, func(path string, entry fs.DirEntry, walkError error) error {
			if walkError != nil {
				return nil
			}
			if path != target && (strings.HasPrefix(entry.Name(), ".") || entry.Name() == "DS_Store") {
				if entry.IsDir() {
					return filepath.SkipDir
				}
				return nil
			}
			if entry.Type().IsRegular() && filepath.Ext(path) == ".md" {
				paths = append(paths, path)
			}
			return nil
		})
	}
	for _, path := range paths {
		relative, err := filepath.Rel(base, path)
		if err != nil {
			continue
		}
		relative = filepath.ToSlash(relative)
		fail := func(reason string) {
			r.Failures = append(r.Failures, model.TermLintFailure{Path: relative, Reason: reason})
		}
		if !directory {
			r.ScannedFiles++
		}
		raw, err := os.ReadFile(path)
		if err != nil {
			fail("io error: " + err.Error())
			continue
		}
		if !utf8.Valid(raw) {
			fail("io error: stream did not contain valid UTF-8")
			continue
		}
		content := string(raw)
		var scanned ScanResult
		if directory {
			scanned, err = Scan(content, relative, terms, options.Selection)
		} else {
			scanned, err = ScanFile(content, relative, terms, options.Selection)
		}
		if err != nil {
			return r, err
		}
		if scanned.Skipped {
			r.SkippedFiles = append(r.SkippedFiles, relative)
			continue
		}
		if directory {
			r.ScannedFiles++
		}
		r.Findings = append(r.Findings, scanned.Findings...)
		for _, f := range scanned.Findings {
			switch f.Selection {
			case "selected":
				r.SelectedCount++
			case "excluded_term", "excluded_original":
				r.ExcludedCount++
			case "below_confidence":
				r.BelowConfidenceCount++
			default:
				r.IneligibleCount++
			}
			if f.HeldBack {
				r.HeldBackCount++
			}
			if f.QuoteProtected {
				r.QuoteProtectedCount++
			}
		}
		if !options.Fix || len(scanned.Fixes) == 0 {
			continue
		}
		count := uint64(len(scanned.Fixes))
		if options.DryRun {
			*r.WouldFixCount += count
			continue
		}
		if !directory {
			count = uint64(len(chosenSpans(content, scanned.Fixes)))
		}
		if directory {
			if _, err := store.CanonicalWithin(target, path); err != nil {
				fail(fmt.Sprintf("path escapes %s/", filepath.ToSlash(target)))
				continue
			}
		}
		if err := (store.AtomicWriter{}).Write(path, []byte(Apply(content, scanned.Fixes)), 0644); err != nil {
			fail("io error: " + err.Error())
			continue
		}
		r.FixedCount += count
		r.ModifiedFiles = append(r.ModifiedFiles, relative)
	}
	if options.Fix || !directory {
		r.WouldApplyCount = r.IneligibleCount
	}
	if directory {
		sort.SliceStable(r.Findings, func(i, j int) bool {
			a, b := r.Findings[i], r.Findings[j]
			if a.Path != b.Path {
				return a.Path < b.Path
			}
			if a.Line != b.Line {
				return a.Line < b.Line
			}
			return a.Column < b.Column
		})
	}
	sort.Strings(r.SkippedFiles)
	sort.Strings(r.ModifiedFiles)
	sort.SliceStable(r.Failures, func(i, j int) bool { return r.Failures[i].Path < r.Failures[j].Path })
	return r, nil
}
