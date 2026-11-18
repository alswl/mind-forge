package lifecycle

import (
	"bytes"
	"path"
	"sort"
	"strings"
	"unicode/utf8"
)

func sameEntry(a Entry, aExists bool, b Entry, bExists bool) bool {
	return aExists == bExists && (!aExists || (a.Kind == b.Kind && a.Mode == b.Mode && bytes.Equal(a.Bytes, b.Bytes)))
}

func (plan Plan) VerifyPartial(current Snapshot) error {
	paths := make(map[string]bool)
	for _, snapshot := range []Snapshot{current, plan.Before, plan.After} {
		for path := range snapshot {
			paths[path] = true
		}
	}
	ordered := make([]string, 0, len(paths))
	for name := range paths {
		ordered = append(ordered, name)
	}
	sort.Strings(ordered)
	for _, path := range ordered {
		actual, exists := current[path]
		before, beforeExists := plan.Before[path]
		after, afterExists := plan.After[path]
		if !sameEntry(actual, exists, before, beforeExists) && !sameEntry(actual, exists, after, afterExists) {
			return rejected("recovery_conflict", "later edit or unknown entry: "+path)
		}
	}
	return nil
}

func relativePath(value string) error {
	if value == "" || strings.HasPrefix(value, "/") || !utf8.ValidString(value) || strings.ContainsRune(value, 0) {
		return rejected("unsafe_path", value)
	}
	for index, component := range strings.Split(value, "/") {
		if component == ".." || component == "." && index == 0 || component == ".git" || component == ".mind-forge" {
			return rejected("unsafe_path", value)
		}
	}
	return nil
}

func (plan Plan) ValidateShape() error {
	if plan.Version != 1 {
		return rejected("invalid_plan", "unsupported version")
	}
	for _, watch := range plan.Watches {
		if watch != "." {
			if err := relativePath(watch); err != nil {
				return err
			}
		}
	}
	for _, snapshot := range []Snapshot{plan.Before, plan.After} {
		folded := make(map[string]bool)
		for _, name := range snapshotNames(snapshot) {
			entry := snapshot[name]
			if err := relativePath(name); err != nil {
				return err
			}
			if entry.Kind != "file" && entry.Kind != "directory" {
				return rejected("invalid_plan", "unknown entry kind "+entry.Kind)
			}
			if entry.Kind == "directory" && entry.Mode&0o700 != 0o700 {
				return rejected("unsupported_permissions", "directory requires owner rwx for recoverable traversal: "+name)
			}
			watched := false
			for _, watch := range plan.Watches {
				if watch == "." || path.Clean(name) == path.Clean(watch) || strings.HasPrefix(path.Clean(name), path.Clean(watch)+"/") {
					watched = true
				}
			}
			if !watched {
				return rejected("invalid_plan", "write escapes watched inventory")
			}
			lower := legacyLower(name)
			if folded[lower] {
				return rejected("path_conflict", name)
			}
			folded[lower] = true
			parentName := path.Dir(name)
			if parentName != "." {
				if parent, exists := snapshot[parentName]; exists {
					if parent.Kind != "directory" {
						return rejected("path_conflict", name)
					}
				} else {
					for _, watch := range plan.Watches {
						if watch == "." || parentName == path.Clean(watch) || strings.HasPrefix(parentName, path.Clean(watch)+"/") {
							return rejected("invalid_plan", "missing parent for "+name)
						}
					}
				}
			}
			if before, exists := plan.Before[name]; exists && before.Kind != entry.Kind {
				return rejected("path_conflict", "file/directory replacement is not supported")
			}
		}
	}
	return nil
}

func snapshotNames(snapshot Snapshot) []string {
	names := make([]string, 0, len(snapshot))
	for name := range snapshot {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
