package report

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	dal "github.com/alswl/mind-forge/pkg/dal/report"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/alswl/mind-forge/pkg/services/session"
)

type Fragment struct {
	Width      int         `json:"width" yaml:"width"`
	Height     int         `json:"height" yaml:"height"`
	Components []Placement `json:"components" yaml:"components"`
}

type Placement struct {
	Entry  string `json:"entry" yaml:"entry"`
	X      int    `json:"x" yaml:"x"`
	Y      int    `json:"y" yaml:"y"`
	Width  int    `json:"width" yaml:"width"`
	Height int    `json:"height" yaml:"height"`
}

func (service Service) BuildFragment(ctx context.Context, scope Scope, slug, name string) (Result, error) {
	if scope.Reports == "" || !ValidSlug(name) || !ValidSlug(slug) {
		return Result{}, fmt.Errorf("%w: expected project, report and fragment slugs", ErrInvalid)
	}
	if err := dal.Local(scope.Assets); err != nil {
		return Result{}, fmt.Errorf("%w: asset layout: %w", ErrInvalid, err)
	}
	root, err := scope.openReport(slug)
	if err != nil {
		return Result{}, err
	}
	defer root.Close()
	file := filepath.Join("fragments", name+".yaml")
	var fragment Fragment
	if err = dal.Manifest(root, file, &fragment); err != nil {
		kind := ErrInvalid
		if errors.Is(err, os.ErrNotExist) {
			kind = ErrNotFound
		}
		return Result{}, fmt.Errorf("%w: %s: %w", kind, file, err)
	}
	if fragment.Width <= 0 || fragment.Height <= 0 || len(fragment.Components) == 0 {
		return Result{}, fmt.Errorf("%w: %s requires a canvas and components", ErrInvalid, file)
	}
	artifact := filepath.Join(scope.Assets, "reports", slug, name+".svg")
	plan, err := service.plan(ctx, root, Request{Slug: slug}, artifact)
	if err != nil {
		return Result{}, err
	}
	if err = validateData(root, plan); err != nil {
		return Result{}, fmt.Errorf("%w: %s: %w", ErrInvalid, plan.Data, err)
	}
	job := plan.job("")
	job.Fragment = &fragment
	for _, p := range fragment.Components {
		if p.Width <= 0 || p.Height <= 0 || p.X < 0 || p.Y < 0 || p.X > fragment.Width-p.Width || p.Y > fragment.Height-p.Height {
			return Result{}, fmt.Errorf("%w: %s: invalid bounds for %s", ErrInvalid, file, p.Entry)
		}
		if err := dal.Regular(root, p.Entry); err != nil {
			return Result{}, fmt.Errorf("%w: %s: %w", ErrInvalid, file, err)
		}
	}
	if service.Worker == nil {
		return Result{}, ErrUnavailable
	}
	stage, err := dal.NewStage(root)
	if err != nil {
		return Result{}, err
	}
	defer stage.Close()
	job.Stage = stage.Path()
	if err = service.run(ctx, job, plan.Result); err != nil {
		return Result{}, err
	}
	raw, err := stage.Bytes()
	if err != nil {
		return Result{}, err
	}
	raw, err = normalizeSVG(raw, fragment)
	if err != nil {
		return Result{}, fmt.Errorf("%w: %s: %w", ErrInvalid, file, err)
	}
	lock, err := session.Begin(ctx, scope.boundary, true, false)
	if err != nil {
		return Result{}, fmt.Errorf("%w: %w", ErrConflict, err)
	}
	defer lock.Close()
	project, err := scope.open()
	if err != nil {
		return Result{}, err
	}
	defer project.Close()
	if err = ctx.Err(); err != nil {
		return Result{}, err
	}
	if err = (asset.Service{}).PublishGenerated(project, plan.Artifact, "report-fragment/"+slug+"/"+name, raw); err != nil {
		if errors.Is(err, asset.ErrGeneratedConflict) {
			return Result{}, fmt.Errorf("%w: %w", ErrConflict, err)
		}
		return Result{}, err
	}
	return plan.Result, nil
}
