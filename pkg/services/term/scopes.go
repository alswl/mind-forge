package term

import (
	"encoding/json"
	"path/filepath"
	"slices"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/textcase"
)

type ScopedTerm struct {
	model.Term
	Identity string  `json:"identity"`
	Scope    *string `json:"scope,omitempty"`
}

func (item ScopedTerm) MarshalJSON() ([]byte, error) {
	raw, err := json.Marshal(item.Term)
	if err != nil {
		return nil, err
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil {
		return nil, err
	}
	fields["identity"], err = json.Marshal(item.Identity)
	if err != nil {
		return nil, err
	}
	if item.Scope != nil {
		fields["scope"], err = json.Marshal(*item.Scope)
		if err != nil {
			return nil, err
		}
	}
	return json.Marshal(fields)
}

type ListFilter struct {
	Filter, Scope string
	Tags, Aliases []string
	HasCorrection bool
}

func List(repo, project string, filter ListFilter) ([]ScopedTerm, error) {
	items := []ScopedTerm{}
	seen := map[string]bool{}
	scopes := []Scope{}
	if project != "" && filter.Scope != "global" {
		scopes = append(scopes, Scope{Root: project, Kind: "project"})
	}
	if project == "" || filter.Scope != "project" {
		scopes = append(scopes, Scope{Root: repo, Kind: "global"})
	}
	for _, scope := range scopes {
		terms, err := scope.Load(true)
		if err != nil {
			return nil, err
		}
		for _, tm := range terms {
			matches := strings.Contains(textcase.Lower(tm.Term), textcase.Lower(filter.Filter))
			for _, v := range append(append([]string{}, tm.Aliases...), tm.Tags...) {
				matches = matches || strings.Contains(textcase.Lower(v), textcase.Lower(filter.Filter))
			}
			if !matches || project != "" && scope.Kind == "global" && seen[tm.Term] {
				continue
			}
			seen[tm.Term] = true
			var label *string
			if project != "" {
				label = stringPtr(scope.Kind)
			}
			items = append(items, ScopedTerm{Term: tm, Identity: tm.Term, Scope: label})
		}
	}
	items = slices.DeleteFunc(items, func(item ScopedTerm) bool {
		return len(filter.Tags) > 0 && !slices.ContainsFunc(filter.Tags, func(v string) bool { return slices.Contains(item.Tags, v) }) || len(filter.Aliases) > 0 && !slices.ContainsFunc(filter.Aliases, func(v string) bool { return slices.Contains(item.Aliases, v) }) || filter.HasCorrection && len(item.Corrections) == 0
	})
	slices.SortStableFunc(items, func(a, b ScopedTerm) int { return strings.Compare(a.Term.Term, b.Term.Term) })
	return items, nil
}

func MergeTerms(project, global []model.Term) []model.Term {
	merged := append([]model.Term{}, project...)
	seen := map[string]bool{}
	for _, tm := range project {
		seen[tm.Term] = true
	}
	for _, tm := range global {
		if !seen[tm.Term] {
			merged = append(merged, tm)
		}
	}
	return merged
}

func (s Scope) ProjectName() string { return filepath.Base(s.Root) }
