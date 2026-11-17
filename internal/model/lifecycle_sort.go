package model

import "sort"

func SortReferences(references []Reference) {
	sort.SliceStable(references, func(a, b int) bool {
		left, right := references[a], references[b]
		if left.FromPath == nil || right.FromPath == nil {
			if (left.FromPath == nil) != (right.FromPath == nil) {
				return left.FromPath == nil
			}
		} else if *left.FromPath != *right.FromPath {
			return *left.FromPath < *right.FromPath
		}
		return left.FromID < right.FromID
	})
}

func SortPlannedChanges(changes []PlannedChange) {
	order := map[string]int{"remove_file": 0, "remove_dir": 1, "rename_file": 2, "rename_dir": 3, "update_yaml": 4, "refresh_index": 5}
	sort.SliceStable(changes, func(a, b int) bool {
		if changes[a].Path != changes[b].Path {
			return changes[a].Path < changes[b].Path
		}
		return order[changes[a].Op] < order[changes[b].Op]
	})
}
