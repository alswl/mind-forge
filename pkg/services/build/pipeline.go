package build

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode"

	"github.com/alswl/mind-forge/pkg/common/logging"
	"github.com/alswl/mind-forge/pkg/common/task"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type PipelineStage struct {
	Rule   string `json:"rule"`
	Input  string `json:"input"`
	Output string `json:"output"`
	Stale  bool   `json:"stale"`
	Action string `json:"action"`
}

type PipelineRunner func(command, input, output string) error

func ValidatePipeline(rules []model.BuildPipelineRule) error {
	names, inputs := map[string]bool{}, map[string]bool{}
	for _, r := range rules {
		if strings.TrimSpace(r.Name) == "" || names[r.Name] {
			return output.Usage("build.pipeline rule names must be unique and non-empty: '"+r.Name+"'", "")
		}
		names[r.Name] = true
		for _, v := range []struct{ name, value string }{{"input_extension", r.InputExtension}, {"output_extension", r.OutputExtension}} {
			if v.value == "" || strings.HasPrefix(v.value, ".") || strings.ContainsAny(v.value, "/\\") || strings.ContainsFunc(v.value, unicode.IsSpace) {
				return output.Usage("build.pipeline."+v.name+" must be a safe extension without a dot, separator, or whitespace", "")
			}
		}
		if strings.TrimSpace(r.Command) == "" || !strings.Contains(r.Command, "{input}") || !strings.Contains(r.Command, "{output}") {
			return output.Usage("build.pipeline rule '"+r.Name+"' command must contain {input} and {output}", "")
		}
		if inputs[r.InputExtension] {
			return output.Usage("build.pipeline has multiple rules for input extension '"+r.InputExtension+"'", "")
		}
		inputs[r.InputExtension] = true
	}
	_, err := pipelineOrder(rules)
	return err
}

func pipelineOrder(rules []model.BuildPipelineRule) ([]int, error) {
	inputs := map[string]int{}
	indegree := make([]int, len(rules))
	edges := make([][]int, len(rules))
	for i, r := range rules {
		inputs[r.InputExtension] = i
	}
	for i, r := range rules {
		if next, ok := inputs[r.OutputExtension]; ok {
			edges[i] = append(edges[i], next)
			indegree[next]++
		}
	}
	queue := []int{}
	for i, n := range indegree {
		if n == 0 {
			queue = append(queue, i)
		}
	}
	order := []int{}
	for len(queue) > 0 {
		i := queue[0]
		queue = queue[1:]
		order = append(order, i)
		for _, next := range edges[i] {
			indegree[next]--
			if indegree[next] == 0 {
				queue = append(queue, next)
			}
		}
	}
	if len(order) != len(rules) {
		return nil, output.Usage("build.pipeline extension graph contains a cycle", "")
	}
	return order, nil
}

func expandPipelineCommand(template, input, outputPath string, dark bool) string {
	quote := func(path string) string { return "'" + strings.ReplaceAll(path, "'", "'\\''") + "'" }
	value := "false"
	if dark {
		value = "true"
	}
	return strings.NewReplacer("{input}", quote(input), "{output}", quote(outputPath), "{dark}", value).Replace(template)
}

func pipelinePath(root, target string) (string, error) {
	if info, err := os.Lstat(target); err == nil && info.Mode()&os.ModeSymlink != 0 {
		return "", output.Usage("pipeline path '"+target+"' must not be a symlink", "")
	}
	path, err := store.CanonicalWithin(root, target)
	if err != nil {
		return "", output.Usage("pipeline path '"+target+"' escapes assets", "")
	}
	return path, nil
}

func pipelineRelative(root, path string) string {
	rel, err := filepath.Rel(root, path)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return filepath.ToSlash(path)
	}
	return filepath.ToSlash(rel)
}

// RunPipeline owns stage ordering, staleness and restoration. The runner is
// injectable so component tests never need to launch a real subprocess.
func RunPipeline(project, assets string, rules []model.BuildPipelineRule, dry bool, runner PipelineRunner) ([]PipelineStage, []string, error) {
	return RunPipelineContext(context.Background(), project, assets, rules, dry, runner)
}

func RunPipelineContext(ctx context.Context, project, assets string, rules []model.BuildPipelineRule, dry bool, runner PipelineRunner) ([]PipelineStage, []string, error) {
	stages, warnings := []PipelineStage{}, []string{}
	if err := ctx.Err(); err != nil {
		return stages, warnings, err
	}
	if err := ValidatePipeline(rules); err != nil {
		return stages, warnings, err
	}
	if len(rules) == 0 {
		return stages, warnings, nil
	}
	if _, err := os.Stat(assets); os.IsNotExist(err) {
		return stages, warnings, nil
	} else if err != nil {
		return stages, warnings, err
	}
	root, err := filepath.EvalSymlinks(assets)
	if err != nil {
		return stages, warnings, err
	}
	root, err = filepath.Abs(root)
	if err != nil {
		return stages, warnings, err
	}
	order, _ := pipelineOrder(rules)
	virtual, regenerated, failed := map[string]bool{}, map[string]bool{}, map[string]bool{}
	err = filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkError error) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		if walkError != nil {
			return walkError
		}
		if entry.Type().IsRegular() {
			checked, err := pipelinePath(root, path)
			if err != nil {
				return err
			}
			virtual[checked] = true
		}
		return nil
	})
	if err != nil {
		return stages, warnings, err
	}
	if runner == nil {
		runner = func(command, _, _ string) error {
			cmd := task.Command(ctx, "sh", "-c", command)
			cmd.Stdin = os.Stdin
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			return cmd.Run()
		}
	}
	collisions := map[string]string{}
	for _, index := range order {
		r := rules[index]
		inputs := []string{}
		for path := range virtual {
			if filepath.Ext(path) == "."+r.InputExtension {
				inputs = append(inputs, path)
			}
		}
		sort.Strings(inputs)
		for _, input := range inputs {
			if err := ctx.Err(); err != nil {
				return stages, warnings, err
			}
			input, err := pipelinePath(root, input)
			if err != nil {
				return stages, warnings, err
			}
			outputPath := strings.TrimSuffix(input, filepath.Ext(input)) + "." + r.OutputExtension
			outputPath, err = pipelinePath(root, outputPath)
			if err != nil {
				return stages, warnings, err
			}
			if previous, ok := collisions[outputPath]; ok {
				return stages, warnings, output.Usage(fmt.Sprintf("build.pipeline output collision: '%s' and '%s' produce %s", previous, r.Name, outputPath), "")
			}
			collisions[outputPath] = r.Name
			outputInfo, statErr := os.Stat(outputPath)
			exists := statErr == nil
			if statErr != nil && !os.IsNotExist(statErr) {
				return stages, warnings, statErr
			}
			stale := regenerated[input] || !exists
			if !stale {
				inputInfo, err := os.Stat(input)
				if err != nil {
					return stages, warnings, err
				}
				stale = inputInfo.ModTime().After(outputInfo.ModTime())
			}
			action := "skip (fresh)"
			if failed[input] {
				action = "skipped (dependency failed)"
			} else if stale {
				action = "run"
			}
			stages = append(stages, PipelineStage{Rule: r.Name, Input: pipelineRelative(project, input), Output: pipelineRelative(project, outputPath), Stale: stale, Action: action})
			virtual[outputPath] = true
			if failed[input] || !stale {
				continue
			}
			if dry {
				regenerated[outputPath] = true
				continue
			}
			var prior []byte
			if exists {
				prior, err = os.ReadFile(outputPath)
				if err != nil {
					return stages, warnings, err
				}
			}
			logging.FromContext(ctx).DebugContext(ctx, "running pipeline stage", "rule", r.Name)
			if err := runner(expandPipelineCommand(r.Command, input, outputPath, r.Dark), input, outputPath); err != nil {
				if exists {
					if err := os.WriteFile(outputPath, prior, outputInfo.Mode().Perm()); err != nil {
						return stages, warnings, err
					}
				} else if err := os.Remove(outputPath); err != nil && !os.IsNotExist(err) {
					return stages, warnings, err
				}
				if err := ctx.Err(); err != nil {
					return stages, warnings, err
				}
				failed[outputPath] = true
				warnings = append(warnings, fmt.Sprintf("pipeline stage '%s' failed; kept existing output %s", r.Name, outputPath))
			} else {
				if err := ctx.Err(); err != nil {
					return stages, warnings, err
				}
				regenerated[outputPath] = true
			}
		}
	}
	for _, stage := range stages {
		if stage.Action == "skipped (dependency failed)" {
			warnings = append(warnings, "pipeline stage '"+stage.Rule+"' skipped because its input stage failed")
		}
	}
	return stages, warnings, nil
}
