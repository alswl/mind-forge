package build

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/model"
)

func pipelineRule(name, input, output string) model.BuildPipelineRule {
	return model.BuildPipelineRule{Name: name, InputExtension: input, OutputExtension: output, Command: "cp {input} {output}"}
}

func TestPipelinePlanningOrdersVirtualChainWithoutWrites(t *testing.T) {
	root := t.TempDir()
	assets := filepath.Join(root, "assets")
	if err := os.Mkdir(assets, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(assets, "flow.d2"), []byte("diagram"), 0644); err != nil {
		t.Fatal(err)
	}
	rules := []model.BuildPipelineRule{pipelineRule("svg-png", "svg", "png"), pipelineRule("d2-svg", "d2", "svg")}
	stages, warnings, err := RunPipeline(root, assets, rules, true, func(string, string, string) error { t.Fatal("preview ran subprocess callback"); return nil })
	if err != nil || len(stages) != 2 || len(warnings) != 0 || stages[0].Rule != "d2-svg" || stages[1].Rule != "svg-png" || stages[1].Action != "run" {
		t.Fatalf("plan: %+v %v %v", stages, warnings, err)
	}
	if _, err := os.Stat(filepath.Join(assets, "flow.svg")); !os.IsNotExist(err) {
		t.Fatal("preview materialized output")
	}
}

func TestPipelineFailureRestoresOutputAndBlocksDownstream(t *testing.T) {
	root := t.TempDir()
	assets := filepath.Join(root, "assets")
	if err := os.Mkdir(assets, 0755); err != nil {
		t.Fatal(err)
	}
	input, svg := filepath.Join(assets, "flow.d2"), filepath.Join(assets, "flow.svg")
	if err := os.WriteFile(svg, []byte("accepted"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(input, []byte("new"), 0644); err != nil {
		t.Fatal(err)
	}
	future := time.Now().Add(time.Hour)
	if err := os.Chtimes(input, future, future); err != nil {
		t.Fatal(err)
	}
	calls := 0
	stages, warnings, err := RunPipeline(root, assets, []model.BuildPipelineRule{pipelineRule("first", "d2", "svg"), pipelineRule("second", "svg", "png")}, false, func(_ string, _ string, output string) error {
		calls++
		if err := os.WriteFile(output, []byte("partial"), 0644); err != nil {
			t.Fatal(err)
		}
		return errors.New("synthetic stage failure")
	})
	got, _ := os.ReadFile(svg)
	if err != nil || calls != 1 || string(got) != "accepted" || len(warnings) != 2 || stages[1].Action != "skipped (dependency failed)" {
		t.Fatalf("failure: %d %q %+v %v %v", calls, got, stages, warnings, err)
	}
}

func TestPipelineRegeneratedInputOverridesFutureDownstreamMtime(t *testing.T) {
	root := t.TempDir()
	assets := filepath.Join(root, "assets")
	if err := os.Mkdir(assets, 0755); err != nil {
		t.Fatal(err)
	}
	for name, body := range map[string]string{"flow.d2": "fresh source", "flow.svg": "old svg", "flow.png": "old png"} {
		if err := os.WriteFile(filepath.Join(assets, name), []byte(body), 0644); err != nil {
			t.Fatal(err)
		}
	}
	future := time.Now().Add(time.Hour)
	if err := os.Chtimes(filepath.Join(assets, "flow.png"), future, future); err != nil {
		t.Fatal(err)
	}
	later := future.Add(time.Hour)
	if err := os.Chtimes(filepath.Join(assets, "flow.d2"), later, later); err != nil {
		t.Fatal(err)
	}
	stages, _, err := RunPipeline(root, assets, []model.BuildPipelineRule{pipelineRule("first", "d2", "svg"), pipelineRule("second", "svg", "png")}, false, func(_ string, input, output string) error {
		raw, err := os.ReadFile(input)
		if err != nil {
			return err
		}
		return os.WriteFile(output, raw, 0644)
	})
	got, _ := os.ReadFile(filepath.Join(assets, "flow.png"))
	if err != nil || string(got) != "fresh source" || stages[1].Action != "run" {
		t.Fatalf("chain: %q %+v %v", got, stages, err)
	}
}

func TestPipelineValidationAndOutputSymlink(t *testing.T) {
	for _, rules := range [][]model.BuildPipelineRule{{pipelineRule("one", "a", "b"), pipelineRule("two", "b", "a")}, {pipelineRule("one", "a", "b"), pipelineRule("two", "a", "c")}, {pipelineRule("one", "../a", "b")}} {
		if err := ValidatePipeline(rules); err == nil {
			t.Fatalf("invalid rules accepted: %+v", rules)
		}
	}
	root := t.TempDir()
	assets := filepath.Join(root, "assets")
	if err := os.Mkdir(assets, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(assets, "flow.svg"), []byte("source"), 0644); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), "outside.png")
	if err := os.WriteFile(outside, []byte("untouched"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(assets, "flow.png")); err != nil {
		t.Fatal(err)
	}
	_, _, err := RunPipeline(root, assets, []model.BuildPipelineRule{pipelineRule("one", "svg", "png")}, false, func(string, string, string) error { t.Fatal("unsafe output executed"); return nil })
	if err == nil {
		t.Fatal("symlink output accepted")
	}
	if quoted := expandPipelineCommand("cmd {input} {output} {dark}", "a'b.svg", "x.png", true); !strings.Contains(quoted, "'a'\\''b.svg'") || !strings.HasSuffix(quoted, " true") {
		t.Fatalf("shell quoting: %q", quoted)
	}
}
