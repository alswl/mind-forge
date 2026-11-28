package report

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/alswl/mind-forge/pkg/common/task"
)

const DefaultWorker = "node_modules/.bin/mind-forge-report-worker"

type Module struct {
	ID      string          `json:"id"`
	Version string          `json:"version"`
	Entry   string          `json:"entry"`
	Schema  json.RawMessage `json:"schema"`
}

type Job struct {
	Protocol   string   `json:"protocol"`
	WorkDir    string   `json:"workDir"`
	Entry      string   `json:"entry"`
	Data       string   `json:"data"`
	Renderer   Module   `json:"renderer"`
	Components []Module `json:"components"`
	View       string   `json:"view"`
	Theme      string   `json:"theme"`
	Stage      string   `json:"stage"`
	Artifact   string   `json:"artifact"`
}

type Receipt struct {
	Protocol string `json:"protocol"`
	Stage    string `json:"stage"`
	Result   Result `json:"result"`
}

type Worker interface {
	Run(context.Context, Job) (Receipt, error)
}

func (plan BuildPlan) job(stage string) Job {
	module := func(item Item) Module {
		return Module{ID: item.ID, Version: item.Version, Entry: item.Entry, Schema: item.Schema}
	}
	components := make([]Module, 0, len(plan.Components))
	for _, component := range plan.Components {
		components = append(components, module(component))
	}
	return Job{
		Protocol: Protocol, WorkDir: plan.Directory, Entry: filepath.Join(plan.Directory, plan.Entry),
		Data: filepath.Join(plan.Directory, plan.Data), Renderer: module(plan.Definition),
		Components: components, View: plan.View, Theme: plan.Theme, Stage: stage, Artifact: plan.Artifact,
	}
}

type ProcessWorker struct {
	// Executable is an absolute, operator-selected path. An empty value uses
	// the report working directory's locked package executable.
	Executable string
	Stderr     io.Writer
}

func (worker ProcessWorker) Run(ctx context.Context, job Job) (Receipt, error) {
	if job.Protocol != Protocol {
		return Receipt{}, fmt.Errorf("unsupported worker request protocol %q", job.Protocol)
	}
	executable := worker.Executable
	if executable == "" {
		executable = filepath.Join(job.WorkDir, DefaultWorker)
	}
	if !filepath.IsAbs(executable) {
		return Receipt{}, fmt.Errorf("worker executable must be absolute")
	}
	input, err := json.Marshal(job)
	if err != nil {
		return Receipt{}, err
	}
	var stdout receiptBuffer
	cmd := task.Command(ctx, executable)
	cmd.Dir = job.WorkDir
	cmd.Stdin = bytes.NewReader(append(input, '\n'))
	cmd.Stdout = &stdout
	cmd.Stderr = worker.Stderr
	if cmd.Stderr == nil {
		cmd.Stderr = os.Stderr
	}
	if err := cmd.Run(); err != nil {
		return Receipt{}, fmt.Errorf("execute %s: %w", executable, err)
	}
	decoder := json.NewDecoder(&stdout.Buffer)
	decoder.DisallowUnknownFields()
	var receipt Receipt
	if err := decoder.Decode(&receipt); err != nil {
		return Receipt{}, fmt.Errorf("invalid worker receipt: %w", err)
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return Receipt{}, fmt.Errorf("worker stdout must contain exactly one receipt")
	}
	return receipt, nil
}

type receiptBuffer struct{ bytes.Buffer }

func (buffer *receiptBuffer) Write(p []byte) (int, error) {
	const limit = 64 << 10
	if buffer.Len()+len(p) > limit {
		return 0, fmt.Errorf("worker receipt exceeds %d bytes", limit)
	}
	return buffer.Buffer.Write(p)
}
