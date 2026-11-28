package content

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

const (
	HandoffFound     = "found"
	HandoffMissing   = "missing"
	HandoffInvalid   = "invalid"
	HandoffAmbiguous = "ambiguous"
)

type HandoffLocation struct {
	Path      string `json:"path"`
	StartLine int    `json:"start_line"`
	EndLine   int    `json:"end_line"`
}

type HandoffView struct {
	Status      string                  `json:"status"`
	Record      *markdown.HandoffFields `json:"record"`
	Locations   []HandoffLocation       `json:"locations"`
	Diagnostics []string                `json:"diagnostics"`
}

type OutputHandoffView struct {
	model.Output
	EditablePath string      `json:"editable_path"`
	Handoff      HandoffView `json:"handoff"`
}

func ReadOutputHandoff(ctx context.Context, project string, item model.Output) (OutputHandoffView, error) {
	path, err := CheckedPath(project, item.Path)
	if err != nil {
		return OutputHandoffView{}, err
	}
	path, err = filepath.Abs(path)
	if err != nil {
		return OutputHandoffView{}, err
	}
	info, err := os.Stat(path)
	if err != nil {
		return OutputHandoffView{}, err
	}
	files, err := store.MarkdownSourceFiles(ctx, path, info.IsDir())
	if err != nil {
		return OutputHandoffView{}, err
	}
	view := HandoffView{Status: HandoffMissing, Locations: []HandoffLocation{}, Diagnostics: []string{}}
	var first markdown.HandoffCallout
	for _, file := range files {
		if err := ctx.Err(); err != nil {
			return OutputHandoffView{}, err
		}
		records, err := readHandoffCallouts(file)
		if err != nil {
			return OutputHandoffView{}, err
		}
		for _, record := range records {
			if len(view.Locations) == 0 {
				first = record
			}
			view.Locations = append(view.Locations, HandoffLocation{file, record.StartLine, record.EndLine})
		}
	}
	switch {
	case len(view.Locations) > 1:
		view.Status = HandoffAmbiguous
		view.Diagnostics = []string{"multiple Handoff callouts in the selected Output; keep one authoritative record"}
	case len(first.Diagnostics) > 0:
		view.Status = HandoffInvalid
		view.Diagnostics = first.Diagnostics
	case len(view.Locations) == 1:
		view.Status = HandoffFound
		view.Record = &first.Fields
	}
	return OutputHandoffView{Output: item, EditablePath: path, Handoff: view}, nil
}

func readHandoffCallouts(path string) ([]markdown.HandoffCallout, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("handoff source is not a regular Markdown file: %s", path)
	}
	body, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	if !utf8.Valid(body) {
		return nil, fmt.Errorf("handoff source is not valid UTF-8: %s", path)
	}
	return markdown.HandoffCallouts(string(body)), nil
}
