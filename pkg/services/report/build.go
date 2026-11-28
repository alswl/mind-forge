package report

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"

	dal "github.com/alswl/mind-forge/pkg/dal/report"
	"github.com/santhosh-tekuri/jsonschema/v6"
)

type reportManifest struct {
	Protocol string `yaml:"apiVersion"`
	Kind     string `yaml:"kind"`
	Version  string `yaml:"version"`
	Renderer string `yaml:"renderer"`
	Entry    string `yaml:"entry"`
	Data     string `yaml:"data"`
}

type Request struct {
	Slug     string
	Renderer string
	Entry    string
	Data     string
	View     string
	Theme    string
}

type Result struct {
	Protocol        string `json:"protocol"`
	Renderer        string `json:"renderer"`
	RendererVersion string `json:"rendererVersion"`
	View            string `json:"view"`
	Artifact        string `json:"artifact"`
}

type BuildPlan struct {
	Result
	Directory  string `json:"workDir"`
	Entry      string `json:"entry"`
	Data       string `json:"data"`
	Theme      string `json:"theme"`
	Definition Item   `json:"definition"`
	Components []Item `json:"components"`
}

func validView(view string) bool { return view == "chart" || view == "table" }

func (service Service) Plan(ctx context.Context, scope Scope, request Request) (BuildPlan, error) {
	root, err := scope.openReport(request.Slug)
	if err != nil {
		return BuildPlan{}, err
	}
	defer root.Close()
	return service.plan(ctx, root, request)
}

func (service Service) plan(ctx context.Context, root *os.Root, request Request) (BuildPlan, error) {
	if err := ctx.Err(); err != nil {
		return BuildPlan{}, err
	}
	m, err := loadReport(root, request)
	if err != nil {
		return BuildPlan{}, err
	}
	entry, err := itemFile(root, ".", "entry", m.Entry)
	if err != nil {
		return BuildPlan{}, fmt.Errorf("%w: %s: %w", ErrInvalid, root.Name(), err)
	}
	data, err := itemFile(root, ".", "data", m.Data)
	if err != nil {
		return BuildPlan{}, fmt.Errorf("%w: %s: %w", ErrInvalid, root.Name(), err)
	}
	view, theme := request.View, request.Theme
	if view == "" {
		view = "chart"
	}
	if theme == "" {
		theme = "system"
	}
	if !validView(view) || theme != "system" && theme != "light" && theme != "dark" {
		return BuildPlan{}, fmt.Errorf("%w: unsupported view %q or theme %q", ErrInvalid, view, theme)
	}
	catalog, err := service.discover()
	if err != nil {
		return BuildPlan{}, err
	}
	item, err := catalog.lookup(Renderer, m.Renderer)
	if err != nil {
		return BuildPlan{}, err
	}
	if !slices.Contains(item.Views, view) {
		return BuildPlan{}, fmt.Errorf("%w: renderer %q does not support view %q", ErrInvalid, item.ID, view)
	}
	components := make([]Item, 0, len(item.Components))
	for _, id := range item.Components {
		component, err := catalog.lookup(Component, id)
		if err != nil {
			return BuildPlan{}, err
		}
		components = append(components, service.resolveEntry(component))
	}
	artifact := filepath.Join("outputs", "report."+view+".html")
	if err := checkArtifact(root, artifact); err != nil {
		return BuildPlan{}, fmt.Errorf("%w: %s: %w", ErrInvalid, artifact, err)
	}
	return BuildPlan{
		Result:    Result{Protocol: Protocol, Renderer: item.ID, RendererVersion: item.Version, View: view, Artifact: artifact},
		Directory: root.Name(), Entry: entry, Data: data, Theme: theme,
		Definition: service.resolveEntry(item), Components: components,
	}, nil
}

func (service Service) resolveEntry(item Item) Item {
	if item.Source == "repo" {
		item.Entry = filepath.Join(service.Registry, item.Entry)
	}
	return item
}

func loadReport(root *os.Root, request Request) (reportManifest, error) {
	var m reportManifest
	if request.Entry != "" || request.Data != "" {
		if request.Slug != "" || request.Entry == "" || request.Data == "" {
			return m, fmt.Errorf("%w: --entry and --data are required together for standalone input", ErrInvalid)
		}
		m.Renderer, m.Entry, m.Data = "builtin/brief", request.Entry, request.Data
	} else {
		if err := dal.Manifest(root, "report.yaml", &m); err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				return m, fmt.Errorf("%w: %s/report.yaml", ErrNotFound, root.Name())
			}
			return m, fmt.Errorf("%w: %s/report.yaml: %w", ErrInvalid, root.Name(), err)
		}
		if m.Protocol != Protocol || m.Kind != "Report" || !validID(m.Renderer) {
			return m, fmt.Errorf("%w: %s/report.yaml: expected apiVersion %s, kind Report and a valid renderer ID", ErrInvalid, root.Name(), Protocol)
		}
	}
	if m.Version != "" && !validVersion(m.Version) {
		return m, fmt.Errorf("%w: %s/report.yaml: invalid report version %q", ErrInvalid, root.Name(), m.Version)
	}
	if request.Renderer != "" {
		m.Renderer = request.Renderer
	}
	return m, nil
}

func checkArtifact(root *os.Root, artifact string) error {
	if err := dal.Check(root, artifact, true); err != nil {
		return err
	}
	if info, err := root.Lstat(artifact); err == nil && !info.Mode().IsRegular() {
		return fmt.Errorf("expected a regular output file")
	} else if err != nil && !errors.Is(err, fs.ErrNotExist) {
		return err
	}
	return nil
}

func (service Service) Build(ctx context.Context, scope Scope, request Request) (Result, error) {
	root, err := scope.openReport(request.Slug)
	if err != nil {
		return Result{}, err
	}
	defer root.Close()
	plan, err := service.plan(ctx, root, request)
	if err != nil {
		return Result{}, err
	}
	if err := validateData(root, plan); err != nil {
		return Result{}, fmt.Errorf("%w: renderer %s, %s: %w", ErrInvalid, plan.Renderer, plan.Data, err)
	}
	if service.Worker == nil {
		return Result{}, fmt.Errorf("%w: no worker configured", ErrUnavailable)
	}
	if err := ctx.Err(); err != nil {
		return Result{}, err
	}
	stage, err := dal.NewStage(root)
	if err != nil {
		return Result{}, fmt.Errorf("stage %s: %w", plan.Artifact, err)
	}
	defer stage.Close()
	job := plan.job(stage.Path())
	receipt, err := service.Worker.Run(ctx, job)
	if ctx.Err() != nil {
		return Result{}, ctx.Err()
	}
	if err != nil {
		return Result{}, fmt.Errorf("%w: renderer %s: %w", ErrUnavailable, plan.Renderer, err)
	}
	expected := Receipt{Protocol: Protocol, Stage: job.Stage, Result: plan.Result}
	if receipt != expected {
		return Result{}, fmt.Errorf("%w: renderer %s: receipt does not match the build request", ErrUnavailable, plan.Renderer)
	}
	if err := stage.Publish(ctx, filepath.Base(plan.Artifact)); err != nil {
		if errors.Is(err, dal.ErrInvalidStage) {
			return Result{}, fmt.Errorf("%w: renderer %s: %w", ErrUnavailable, plan.Renderer, err)
		}
		return Result{}, fmt.Errorf("publish %s: %w", plan.Artifact, err)
	}
	return plan.Result, nil
}

func validateData(root *os.Root, plan BuildPlan) error {
	schema, err := compileSchema(plan.Definition.Schema)
	if err != nil {
		return err
	}
	raw, err := dal.Read(root, plan.Data)
	if err != nil {
		return err
	}
	value, err := jsonschema.UnmarshalJSON(bytes.NewReader(raw))
	if err != nil {
		return err
	}
	return schema.Validate(value)
}
