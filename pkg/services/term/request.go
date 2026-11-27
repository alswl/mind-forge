package term

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/alswl/mind-forge/pkg/services/content"
)

func emptyTermReport(fix, dry bool) model.TermLintReport {
	r := model.TermLintReport{Findings: []model.TermFinding{}, SkippedFiles: []string{}, ModifiedFiles: []string{}, Failures: []model.TermLintFailure{}}
	if fix && dry {
		zero := uint64(0)
		r.WouldFixCount = &zero
	}
	return r
}

func LintRequest(ctx context.Context, root, cwd string, scope Scope, args []string, selection Selection, fix, dry bool) (model.TermLintReport, error) {
	empty := emptyTermReport(fix, dry)
	terms, err := scope.Load(true)
	if err != nil {
		return empty, err
	}
	if scope.Kind == "project" {
		global, err := (Scope{Root: root, Kind: "global"}).Load(true)
		if err != nil {
			return empty, err
		}
		terms = MergeTerms(terms, global)
	}
	empty.NoTermsRegistered = len(terms) == 0
	if err := selection.Validate(terms); err != nil {
		return empty, output.Usage(err.Error(), "")
	}
	if scope.Kind == "global" && len(terms) == 0 {
		return empty, nil
	}
	var config *model.MindConfig
	if scope.Kind == "project" && (!fix || len(args) == 0) {
		config, err = content.ConfigFor(scope.Root)
		if err != nil {
			return empty, err
		}
	}
	base, target := scope.Root, scope.Root
	if len(args) > 0 {
		target = args[0]
		if !filepath.IsAbs(target) {
			anchor := scope.Root
			if scope.Kind == "global" {
				for dir := cwd; ; dir = filepath.Dir(dir) {
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
			canonicalTarget, pathErr := store.CanonicalWithin(scope.Root, target)
			if pathErr != nil {
				return empty, termProjectBoundaryError(scope.Root, target, pathErr)
			}
			target = canonicalTarget
		} else if info, err := os.Stat(target); err == nil && info.IsDir() {
			canonical, err := filepath.EvalSymlinks(target)
			if err != nil {
				return empty, err
			}
			target = canonical
			if _, err := store.CanonicalWithin(root, target); err != nil {
				base = target
			}
		}
	} else if scope.Kind == "project" {
		layout, _ := app.ResolveEffectiveLayout(config)
		target = filepath.Join(scope.Root, layout.Articles)
		if _, err := os.Stat(target); os.IsNotExist(err) {
			return empty, nil
		}
	}
	report, err := LintFiles(base, target, terms, LintOptions{Context: ctx, Selection: selection, Fix: fix, DryRun: dry})
	if err == nil && !fix && config != nil {
		suppressHeldBackFindings(&report, config.Term.HeldBackIgnore)
	}
	return report, err
}

func suppressHeldBackFindings(report *model.TermLintReport, ignored []model.HeldBackTermPair) {
	if len(ignored) == 0 || report.HeldBackCount == 0 {
		return
	}
	pairs := make(map[model.HeldBackTermPair]struct{}, len(ignored))
	for _, pair := range ignored {
		pairs[pair] = struct{}{}
	}
	report.Findings = slices.DeleteFunc(report.Findings, func(finding model.TermFinding) bool {
		if !finding.HeldBack {
			return false
		}
		pair := model.HeldBackTermPair{Original: finding.Original, Correct: finding.Correct}
		_, suppressed := pairs[pair]
		if suppressed {
			report.SuppressedHeldBackCount++
		}
		return suppressed
	})
}

func termProjectBoundaryError(project, target string, err error) error {
	var failure *output.Error
	if errors.As(err, &failure) && failure.Kind == "usage" {
		return output.Usage(fmt.Sprintf("path '%s' is not under project root '%s'", target, project), "")
	}
	return err
}
