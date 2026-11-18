package term

import (
	"bytes"
	"fmt"
	"maps"
	"path/filepath"
	"slices"
	"strings"

	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func (s Scope) Rename(old, name string, keepAlias, force bool) error {
	if strings.TrimSpace(old) == "" || strings.TrimSpace(name) == "" {
		return output.Usage("term name cannot be empty", "")
	}
	d, err := s.document(true)
	if err != nil {
		return err
	}
	i, err := findTerm(d.terms, old)
	if err != nil {
		return err
	}
	if old != name && !force && slices.ContainsFunc(d.terms, func(tm model.Term) bool { return tm.Term == name }) {
		return output.Usage("a term named '"+name+"' already exists", "use --force to overwrite")
	}
	tm := &d.terms[i]
	if keepAlias && !slices.Contains(tm.Aliases, old) {
		tm.Aliases = append(tm.Aliases, old)
	}
	tm.Term = name
	raw, err := s.render(d, s.Kind == "global")
	if err != nil {
		return err
	}
	return (store.AtomicWriter{}).Write(s.Path(), raw, 0644)
}

func (s Scope) Remove(name string, force, dry bool) error {
	if strings.TrimSpace(name) == "" {
		return output.Usage("term name cannot be empty", "")
	}
	d, err := s.document(false)
	if err != nil {
		return err
	}
	i, err := findTerm(d.terms, name)
	if err != nil {
		if s.Kind == "global" {
			return output.Usage("term '"+name+"' not found in global terms", "use `mf term list --global` to see available terms")
		}
		return err
	}
	if d.index != nil && !force && d.index.Value.Articles != nil && len(*d.index.Value.Articles) > 0 {
		names := []string{}
		for _, a := range *d.index.Value.Articles {
			names = append(names, a.Title)
		}
		return output.Usage(fmt.Sprintf("term '%s' is referenced by: %s. Use --force to remove anyway.", name, strings.Join(names, ", ")), "check which articles reference this term before removal")
	}
	if s.Kind == "global" {
		d.terms = slices.DeleteFunc(d.terms, func(tm model.Term) bool { return tm.Term == name })
	} else {
		d.terms = slices.Delete(d.terms, i, i+1)
	}
	raw, err := s.render(d, false)
	if err != nil || dry {
		return err
	}
	return (store.AtomicWriter{}).Write(s.Path(), raw, 0644)
}

type MovePlan struct {
	Plan   lifecycle.Plan
	Report model.TermMoveReport
}

func PlanMove(repo string, from, to Scope, name string, force bool) (MovePlan, error) {
	result := MovePlan{}
	src, err := from.document(true)
	if err != nil {
		return result, err
	}
	pos, err := findTerm(src.terms, name)
	if err != nil {
		return result, err
	}
	dst, err := to.document(true)
	if err != nil {
		return result, err
	}
	if !force && slices.ContainsFunc(dst.terms, func(tm model.Term) bool { return tm.Term == name }) {
		return result, output.Usage("term '"+name+"' already exists in destination scope; use --force to overwrite", "")
	}
	root, err := filepath.EvalSymlinks(repo)
	if err != nil {
		return result, err
	}
	root, err = filepath.Abs(root)
	if err != nil {
		return result, err
	}
	watches := []string{}
	for _, scope := range []Scope{from, to} {
		absolute, err := store.CanonicalWithin(root, scope.Path())
		if err != nil {
			return result, err
		}
		rel, err := filepath.Rel(root, absolute)
		if err != nil {
			return result, err
		}
		watches = append(watches, filepath.ToSlash(rel))
	}
	before, err := lifecycle.SnapshotFiles(root, watches)
	if err != nil {
		return result, err
	}
	for i, document := range []*termDocument{src, dst} {
		entry, exists := before[watches[i]]
		if exists != document.existed || exists && !bytes.Equal(entry.Bytes, document.original) {
			return result, &output.Error{Kind: "stale_plan", Lifecycle: true, Message: "term scope changed while planning"}
		}
	}
	after := maps.Clone(before)
	moved := src.terms[pos]
	src.terms = slices.Delete(src.terms, pos, pos+1)
	if force {
		dst.terms = slices.DeleteFunc(dst.terms, func(tm model.Term) bool { return tm.Term == name })
	}
	dst.terms = append(dst.terms, moved)
	for i, d := range []*termDocument{src, dst} {
		scope := from
		if i == 1 {
			scope = to
		}
		raw, err := scope.render(d, true)
		if err != nil {
			return result, err
		}
		entry, ok := after[watches[i]]
		if !ok {
			entry = lifecycle.Entry{Kind: "file", Mode: 0644}
		}
		entry.Bytes = raw
		after[watches[i]] = entry
	}
	uniqueWatches := slices.Compact(append([]string{}, watches...))
	result.Plan, err = lifecycle.NewPlan(root, "move", uniqueWatches, before, after)
	if err != nil {
		return result, err
	}
	removeDescription, addDescription := "removed from "+from.Kind, "added to "+to.Kind
	if from.Kind == "project" && to.Kind == "project" {
		removeDescription = "removed from source project"
		addDescription = "added to destination project"
	}
	result.Report = model.TermMoveReport{Term: name, FromScope: from.Kind, ToScope: to.Kind, Force: force, SideEffects: []model.MoveSideEffect{{Action: "remove", Scope: from.Kind, Description: removeDescription}, {Action: "add", Scope: to.Kind, Description: addDescription}}}
	return result, nil
}
