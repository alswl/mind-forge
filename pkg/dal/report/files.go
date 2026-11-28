package report

import (
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"go.yaml.in/yaml/v3"
)

const maxInput = 16 << 20

// Local checks the manifest spelling before any path cleanup can erase traversal.
func Local(path string) error {
	if !filepath.IsLocal(path) || strings.ContainsAny(path, "\\:\x00") {
		return fmt.Errorf("expected a relative path, got %q", path)
	}
	for _, part := range strings.Split(path, "/") {
		if part == ".." {
			return fmt.Errorf("parent traversal in %q", path)
		}
	}
	return nil
}

// Check rejects symlinks and case aliases along a path, including on
// case-insensitive filesystems. A missing suffix can be allowed for new files.
func Check(root *os.Root, path string, missing bool) error {
	if err := Local(path); err != nil {
		return err
	}
	dir := "."
	for part := range strings.SplitSeq(filepath.Clean(path), string(filepath.Separator)) {
		if part == "." {
			continue
		}
		entries, err := fs.ReadDir(root.FS(), dir)
		if err != nil {
			return err
		}
		var exact, alias bool
		for _, entry := range entries {
			if strings.EqualFold(entry.Name(), part) {
				exact = exact || entry.Name() == part
				alias = alias || entry.Name() != part
			}
		}
		dir = filepath.Join(dir, part)
		if !exact && alias {
			return fmt.Errorf("%s: filename case mismatch", dir)
		}
		info, err := root.Lstat(dir)
		if errors.Is(err, os.ErrNotExist) && missing {
			return nil
		}
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("%s: symlinks are not supported", dir)
		}
	}
	return nil
}

func Regular(root *os.Root, path string) error {
	if err := Check(root, path, false); err != nil {
		return err
	}
	info, err := root.Stat(path)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("%s: expected a regular file", path)
	}
	return nil
}

func Read(root *os.Root, path string) ([]byte, error) {
	if err := Regular(root, path); err != nil {
		return nil, err
	}
	file, err := root.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	raw, err := io.ReadAll(io.LimitReader(file, maxInput+1))
	if err == nil && len(raw) > maxInput {
		return nil, fmt.Errorf("%s: file exceeds %d bytes", path, maxInput)
	}
	return raw, err
}

func Manifest(root *os.Root, path string, dest any) error {
	raw, err := Read(root, path)
	if err != nil {
		return err
	}
	decoder := yaml.NewDecoder(strings.NewReader(string(raw)))
	decoder.KnownFields(true)
	if err := decoder.Decode(dest); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return fmt.Errorf("expected a single YAML document")
	}
	return nil
}
