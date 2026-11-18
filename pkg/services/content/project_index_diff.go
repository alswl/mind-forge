package content

import (
	"sort"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
)

type projectIndexChanges struct {
	added, removed []model.ProjectEntry
	paths          map[string]string
}

func compareProjects(manifest model.Manifest, scanned []ScannedProject, now time.Time) projectIndexChanges {
	changes := projectIndexChanges{added: []model.ProjectEntry{}, removed: []model.ProjectEntry{}, paths: map[string]string{}}
	for _, entry := range scanned {
		changes.paths[entry.Name] = entry.Path
	}
	old := map[string]bool{}
	for _, entry := range manifest.Projects {
		old[entry.Name] = true
	}
	for _, entry := range scanned {
		if !old[entry.Name] {
			changes.added = append(changes.added, model.ProjectEntry{Name: entry.Name, Path: entry.Path, CreatedAt: articleTimestamp(now)})
		}
	}
	for _, entry := range manifest.Projects {
		if _, exists := changes.paths[entry.Name]; !exists {
			changes.removed = append(changes.removed, entry)
		}
	}
	sort.SliceStable(changes.added, func(i, j int) bool { return changes.added[i].Name < changes.added[j].Name })
	return changes
}

func reconcileProjects(manifest model.Manifest, changes projectIndexChanges) model.Manifest {
	removed := map[string]bool{}
	for _, entry := range changes.removed {
		removed[entry.Name] = true
	}
	projects := model.JSONArray[model.ProjectEntry]{}
	for _, entry := range manifest.Projects {
		if removed[entry.Name] {
			continue
		}
		if path, exists := changes.paths[entry.Name]; exists {
			entry.Path = path
		}
		projects = append(projects, entry)
	}
	manifest.Projects = append(projects, changes.added...)
	return manifest
}
