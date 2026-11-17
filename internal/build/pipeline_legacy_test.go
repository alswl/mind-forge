package build

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/model"
)

func TestPipelineFrozenReadOnlyAndRefusalAssertions(t *testing.T) {
	t.Run("orders_chained_rules_and_does_not_run_dry_plan", func(t *testing.T) {
		root := buildTestRoot(t)
		putBuildFile(t, root, "assets/diagram.d2", "d2")
		rules := []model.BuildPipelineRule{pipelineRule("d2-to-svg", "d2", "svg"), pipelineRule("svg-to-png", "svg", "png")}
		stages, _, err := RunPipeline(root, filepath.Join(root, "assets"), rules, true, func(string, string, string) error { t.Fatal("preview invoked its runner"); return nil })
		if err != nil {
			t.Fatal(err)
		}
		names := []string{}
		for _, stage := range stages {
			names = append(names, stage.Rule)
		}
		if !reflect.DeepEqual(names, []string{"d2-to-svg", "svg-to-png"}) {
			t.Fatal(names)
		}
		if _, err := os.Stat(filepath.Join(root, "assets/diagram.svg")); !os.IsNotExist(err) {
			t.Fatalf("preview created svg: %v", err)
		}
	})
	t.Run("rejects_broken_output_symlink_before_running_command", func(t *testing.T) {
		root := buildTestRoot(t)
		putBuildFile(t, root, "assets/diagram.svg", "svg")
		outside := filepath.Join(root, "outside.png")
		if err := os.Symlink(outside, filepath.Join(root, "assets/diagram.png")); err != nil {
			t.Fatal(err)
		}
		_, _, err := RunPipeline(root, filepath.Join(root, "assets"), []model.BuildPipelineRule{pipelineRule("svg-to-png", "svg", "png")}, false, func(string, string, string) error { t.Fatal("unsafe output invoked its runner"); return nil })
		if err == nil || !strings.Contains(err.Error(), "must not be a symlink") {
			t.Fatalf("%v", err)
		}
		if _, err := os.Stat(outside); !os.IsNotExist(err) {
			t.Fatalf("outside output created: %v", err)
		}
	})
}

func TestPipelineFrozenExecutionDecisions(t *testing.T) {
	copyAsset := func(_ string, input, output string) error {
		raw, err := os.ReadFile(input)
		if err != nil {
			return err
		}
		return os.WriteFile(output, raw, 0644)
	}
	t.Run("plans_and_runs_stale_sibling", func(t *testing.T) {
		root := buildTestRoot(t)
		putBuildFile(t, root, "assets/diagram.svg", "svg")
		stages, warnings, err := RunPipeline(root, filepath.Join(root, "assets"), []model.BuildPipelineRule{pipelineRule("svg-to-png", "svg", "png")}, false, copyAsset)
		if err != nil || len(stages) != 1 || stages[0].Action != "run" || len(warnings) != 0 {
			t.Fatalf("%+v %v %v", stages, warnings, err)
		}
		raw, err := os.ReadFile(filepath.Join(root, "assets/diagram.png"))
		if err != nil || string(raw) != "svg" {
			t.Fatalf("%q %v", raw, err)
		}
	})
	t.Run("regenerated_intermediate_makes_downstream_stage_stale", func(t *testing.T) {
		root := buildTestRoot(t)
		source := putBuildFile(t, root, "assets/flow.d2", "initial\n")
		rules := []model.BuildPipelineRule{pipelineRule("d2-to-svg", "d2", "svg"), pipelineRule("svg-to-png", "svg", "png")}
		if _, _, err := RunPipeline(root, filepath.Join(root, "assets"), rules, false, copyAsset); err != nil {
			t.Fatal(err)
		}
		now := time.Now()
		png := filepath.Join(root, "assets/flow.png")
		if err := os.Chtimes(png, now.Add(120*time.Second), now.Add(120*time.Second)); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(source, []byte("stale input\n"), 0644); err != nil {
			t.Fatal(err)
		}
		if err := os.Chtimes(source, now.Add(240*time.Second), now.Add(240*time.Second)); err != nil {
			t.Fatal(err)
		}
		stages, warnings, err := RunPipeline(root, filepath.Join(root, "assets"), rules, false, copyAsset)
		actions := []string{}
		for _, stage := range stages {
			actions = append(actions, stage.Action)
		}
		if err != nil || len(warnings) != 0 || !reflect.DeepEqual(actions, []string{"run", "run"}) {
			t.Fatalf("%v %v %v", actions, warnings, err)
		}
		raw, err := os.ReadFile(png)
		if err != nil || string(raw) != "stale input\n" {
			t.Fatalf("%q %v", raw, err)
		}
	})
}
