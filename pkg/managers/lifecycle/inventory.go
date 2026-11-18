package lifecycle

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func rejected(kind string, reason any) error {
	return &output.Error{Kind: kind, Message: fmt.Sprint(reason), Lifecycle: true}
}

func safePath(root, relative string) (string, error) {
	if err := relativePath(relative); err != nil {
		return "", err
	}
	rootInfo, err := os.Stat(root)
	if err != nil {
		return "", err
	}
	rootProperties, err := fileProperties(rootInfo)
	if err != nil {
		return "", err
	}
	result := root
	for _, part := range strings.Split(relative, "/") {
		result = filepath.Join(result, part)
		info, err := os.Lstat(result)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return "", err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", rejected("unsafe_path", "symlink "+result)
		}
		properties, err := fileProperties(info)
		if err != nil {
			return "", err
		}
		if properties.device != rootProperties.device {
			return "", rejected("cross_device", result)
		}
	}
	return result, nil
}

// SnapshotFiles inspects the watched tree without creating transaction state.
// The caller takes the write lock before using an inventory to publish changes.
func SnapshotFiles(root string, watches []string) (Snapshot, error) {
	return SnapshotFilesContext(context.Background(), root, watches)
}

func SnapshotFilesContext(ctx context.Context, root string, watches []string) (Snapshot, error) {
	root = filepath.Clean(root)
	rootInfo, err := os.Stat(root)
	if err != nil {
		return nil, err
	}
	rootProperties, err := fileProperties(rootInfo)
	if err != nil {
		return nil, err
	}
	result := Snapshot{}
	for _, watch := range watches {
		start := root
		if watch != "." {
			start, err = safePath(root, watch)
			if err != nil {
				return nil, err
			}
		}
		if _, err := os.Lstat(start); errors.Is(err, os.ErrNotExist) {
			continue
		} else if err != nil {
			return nil, err
		}
		err = filepath.WalkDir(start, func(path string, entry fs.DirEntry, walkError error) error {
			if err := ctx.Err(); err != nil {
				return err
			}
			if walkError != nil {
				return rejected("inventory_failed", walkError)
			}
			if path != start && filepath.Dir(path) == start && (entry.Name() == ".git" || entry.Name() == ".mind-forge") {
				if entry.IsDir() {
					return filepath.SkipDir
				}
				return nil
			}
			relative, err := filepath.Rel(root, path)
			if err != nil {
				return rejected("unsafe_path", err)
			}
			if relative == "." {
				return nil
			}
			if !utf8.ValidString(relative) {
				return rejected("unsafe_path", "non UTF-8 path")
			}
			if _, err := safePath(root, relative); err != nil {
				return err
			}
			info, err := os.Lstat(path)
			if err != nil {
				return err
			}
			properties, err := fileProperties(info)
			if err != nil {
				return err
			}
			if properties.device != rootProperties.device {
				return rejected("cross_device", path)
			}
			value := Entry{Mode: properties.mode}
			switch {
			case info.Mode().IsRegular():
				if properties.links != 1 {
					return rejected("unsafe_path", "hardlinked file "+relative)
				}
				value.Kind = "file"
				value.Bytes, err = os.ReadFile(path)
				if err != nil {
					return err
				}
			case info.IsDir():
				value.Kind = "directory"
			default:
				return rejected("unsafe_path", "non-regular entry "+relative)
			}
			result[filepath.ToSlash(relative)] = value
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	return result, nil
}
