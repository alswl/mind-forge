package term

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestFileLintPreviewAndApplyPreserveSourcesUntilCommit(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "doc.md")
	original := "api `api`\n> api\n"
	if err := os.WriteFile(target, []byte(original), 0640); err != nil {
		t.Fatal(err)
	}
	terms := []model.Term{termRule("API", "api", "API", "word", "standalone")}
	r, err := LintFiles(root, target, terms, LintOptions{Fix: true, DryRun: true})
	if err != nil || r.FixedCount != 0 || r.WouldFixCount == nil || *r.WouldFixCount != 1 || r.QuoteProtectedCount != 1 || r.IneligibleCount != 1 {
		t.Fatalf("preview: %+v %v", r, err)
	}
	raw, _ := os.ReadFile(target)
	if string(raw) != original {
		t.Fatal("preview wrote source")
	}
	r, err = LintFiles(root, target, terms, LintOptions{Fix: true})
	if err != nil || r.FixedCount != 1 || len(r.ModifiedFiles) != 1 || len(r.Failures) != 0 {
		t.Fatalf("apply: %+v %v", r, err)
	}
	raw, _ = os.ReadFile(target)
	info, _ := os.Stat(target)
	if string(raw) != "API `api`\n> api\n" || info.Mode().Perm() != 0640 {
		t.Fatalf("source preservation: %q %v", raw, info.Mode())
	}
}

func TestDirectoryLintSkipsFrontmatterHiddenAndSymlinkFiles(t *testing.T) {
	root := t.TempDir()
	outside := filepath.Join(t.TempDir(), "outside.md")
	for name, content := range map[string]string{"a.md": "api\n", "b.md": "---\nmf-term-lint: skip\n---\napi\n", ".hidden.md": "api\n", "other.txt": "api\n"} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(outside, []byte("api\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "escape.md")); err != nil {
		t.Fatal(err)
	}
	r, err := LintFiles(root, root, []model.Term{termRule("API", "api", "API", "word", "standalone")}, LintOptions{Fix: true})
	if err != nil || r.ScannedFiles != 1 || len(r.SkippedFiles) != 1 || r.SkippedFiles[0] != "b.md" || r.FixedCount != 1 {
		t.Fatalf("directory: %+v %v", r, err)
	}
	raw, _ := os.ReadFile(outside)
	if strings.TrimSpace(string(raw)) != "api" {
		t.Fatal("followed symlink")
	}
}

func TestLintReportSelectionCounters(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "a.md")
	if err := os.WriteFile(target, []byte("机器 api oneoff\n"), 0644); err != nil {
		t.Fatal(err)
	}
	terms := []model.Term{termRule("装置", "机器", "装置", "word", "standalone"), termRule("API", "api", "API", "word", "standalone")}
	r, err := LintFiles(root, target, terms, LintOptions{Selection: Selection{ExcludeOriginals: []string{"api"}, AdHoc: []model.Correction{{Original: "oneoff", Correct: "stable"}}}})
	if err != nil || len(r.Findings) != 3 || r.SelectedCount != 1 || r.ExcludedCount != 1 || r.HeldBackCount != 1 || r.IneligibleCount != 1 || r.WouldApplyCount != 1 {
		t.Fatalf("single-file counters: %+v %v", r, err)
	}
	r, err = LintFiles(root, root, terms, LintOptions{Selection: Selection{ExcludeOriginals: []string{"api"}, AdHoc: []model.Correction{{Original: "oneoff", Correct: "stable"}}}})
	if err != nil || r.WouldApplyCount != 0 || r.FixedCount != 0 {
		t.Fatalf("directory read-only counters: %+v %v", r, err)
	}
}

func TestExplicitFileCountsOnlyAppliedOverlappingPinyinSpans(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "a.md")
	if err := os.WriteFile(target, []byte("实实实\n"), 0644); err != nil {
		t.Fatal(err)
	}
	terms := []model.Term{termRule("时时", "时时", "时时", "pinyin", "loose")}
	r, err := LintFiles(root, target, terms, LintOptions{Fix: true, Selection: Selection{IncludeSuggested: true}})
	if err != nil || len(r.Findings) != 2 || r.FixedCount != 1 {
		t.Fatalf("overlapping explicit-file count: %+v %v", r, err)
	}
	raw, _ := os.ReadFile(target)
	if string(raw) != "时时实\n" {
		t.Fatalf("overlap applied twice: %q", raw)
	}
}

func TestLintFileProvenanceUsesTheSamePhysicalRoot(t *testing.T) {
	for _, directory := range []bool{false, true} {
		for _, dry := range []bool{true, false} {
			for _, baseAlias := range []bool{true, false} {
				t.Run(fmt.Sprintf("directory=%v/dry=%v/baseAlias=%v", directory, dry, baseAlias), func(t *testing.T) {
					root := t.TempDir()
					physical, err := filepath.EvalSymlinks(root)
					if err != nil {
						t.Fatal(err)
					}
					alias := filepath.Join(t.TempDir(), "alias")
					if err := os.Symlink(physical, alias); err != nil {
						t.Fatal(err)
					}
					if err := os.Mkdir(filepath.Join(physical, "docs"), 0755); err != nil {
						t.Fatal(err)
					}
					if err := os.WriteFile(filepath.Join(physical, "docs", "doc.md"), []byte("api\n"), 0640); err != nil {
						t.Fatal(err)
					}
					base, target := physical, alias
					if baseAlias {
						base, target = alias, physical
					}
					target = filepath.Join(target, "docs")
					if !directory {
						target = filepath.Join(target, "doc.md")
					}
					report, err := LintFiles(base, target, []model.Term{termRule("API", "api", "API", "word", "standalone")}, LintOptions{Fix: true, DryRun: dry})
					if err != nil {
						t.Fatal(err)
					}
					if len(report.Findings) != 1 || report.Findings[0].Path != "docs/doc.md" {
						t.Fatalf("path provenance: %+v", report)
					}
					if !dry && (len(report.ModifiedFiles) != 1 || report.ModifiedFiles[0] != "docs/doc.md") {
						t.Fatalf("modified provenance: %+v", report)
					}
					raw, err := os.ReadFile(filepath.Join(physical, "docs", "doc.md"))
					if err != nil {
						t.Fatal(err)
					}
					want := "API\n"
					if dry {
						want = "api\n"
					}
					if string(raw) != want {
						t.Fatalf("content=%q want %q", raw, want)
					}
				})
			}
		}
	}
}
