package content

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/alswl/mind-forge/pkg/common/textcase"
)

type OutputCreateOptions struct {
	Title        *string
	File, DryRun bool
	Now          time.Time
}

func CreateOutput(root, project, selector, name string, options OutputCreateOptions) (map[string]any, error) {
	if err := ValidateName(name); err != nil {
		return nil, err
	}
	mind, err := ResolveMind(root, project, selector)
	if err != nil {
		return nil, err
	}
	for _, item := range mind.Outputs {
		if textcase.ASCIIEqualFold(item.Name, name) {
			return nil, fileExists(filepath.Join(project, name))
		}
	}
	config, err := ConfigFor(project)
	if err != nil {
		return nil, err
	}
	base := OutputRoot(*config.ContentLayout, mind.Name) + "/" + name
	relative, shape := base, "directory"
	if options.File {
		relative += ".md"
		shape = "file"
	}
	path, err := CheckedPath(project, relative)
	if err != nil {
		return nil, err
	}
	for _, candidate := range []string{base, base + ".md"} {
		checked, err := CheckedPath(project, candidate)
		if err != nil {
			return nil, err
		}
		if _, err := os.Stat(checked); err == nil {
			return nil, fileExists(filepath.Join(project, candidate))
		} else if !os.IsNotExist(err) {
			return nil, err
		}
	}
	if !options.DryRun {
		destination := path
		if !options.File {
			destination = filepath.Join(path, "01-opening.md")
		}
		if err := os.MkdirAll(filepath.Dir(destination), 0o777); err != nil {
			return nil, err
		}
		file, err := os.OpenFile(destination, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o666)
		if err != nil {
			return nil, err
		}
		title := name
		if options.Title != nil {
			title = *options.Title
		}
		text := fmt.Sprintf("# %s\n\n", title)
		count, writeErr := io.WriteString(file, text)
		closeErr := file.Close()
		if writeErr != nil {
			return nil, writeErr
		}
		if count != len(text) {
			return nil, io.ErrShortWrite
		}
		if closeErr != nil {
			return nil, closeErr
		}
	}
	var actual any
	if !options.DryRun {
		actual = relative
	}
	return map[string]any{"kind": "output", "identity": mind.Identity + "::" + name, "created_at": creationTime(options.Now), "dry_run": options.DryRun, "path": actual, "details": map[string]any{"proposed_path": relative, "shape": shape, "mind_identity": mind.Identity}}, nil
}
