package report

import (
	"context"
	"crypto/rand"
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

var ErrInvalidStage = errors.New("invalid report staging file")

type Stage struct {
	dir      *os.Root
	name     string
	original *os.File
}

func NewStage(root *os.Root) (*Stage, error) {
	const dirName = "outputs"
	if err := Check(root, dirName, true); err != nil {
		return nil, err
	}
	if err := root.Mkdir(dirName, 0755); err != nil && !errors.Is(err, os.ErrExist) {
		return nil, err
	}
	if err := Check(root, dirName, false); err != nil {
		return nil, err
	}
	dir, err := root.OpenRoot(dirName)
	if err != nil {
		return nil, err
	}
	name := ".mf-report-" + rand.Text() + ".tmp"
	file, err := dir.OpenFile(name, os.O_RDWR|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		dir.Close()
		return nil, err
	}
	return &Stage{dir: dir, name: name, original: file}, nil
}

func (stage *Stage) Path() string { return filepath.Join(stage.dir.Name(), stage.name) }

func (stage *Stage) Close() {
	_ = stage.dir.RemoveAll(stage.name)
	_ = stage.original.Close()
	_ = stage.dir.Close()
}

// validate verifies the directory and file identities before the atomic rename.
func (stage *Stage) validate() error {
	for _, path := range []string{stage.dir.Name(), filepath.Dir(stage.dir.Name())} {
		info, err := os.Lstat(path)
		if err != nil || !info.IsDir() {
			return fmt.Errorf("output path changed: %s", path)
		}
	}
	current, err := os.Stat(stage.dir.Name())
	if err != nil {
		return err
	}
	originalDir, err := stage.dir.Stat(".")
	if err != nil || !os.SameFile(current, originalDir) {
		return fmt.Errorf("output directory changed")
	}
	info, err := stage.dir.Lstat(stage.name)
	if err != nil {
		return fmt.Errorf("%w: %w", ErrInvalidStage, err)
	}
	originalFile, err := stage.original.Stat()
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Size() == 0 || !os.SameFile(info, originalFile) {
		return fmt.Errorf("%w: worker must write nonempty output to the original staging file", ErrInvalidStage)
	}
	return nil
}
func (stage *Stage) Bytes() ([]byte, error) {
	if err := stage.validate(); err != nil {
		return nil, err
	}
	info, err := stage.original.Stat()
	if err != nil {
		return nil, err
	}
	if info.Size() > maxInput {
		return nil, fmt.Errorf("SVG exceeds %d bytes", maxInput)
	}
	raw := make([]byte, info.Size())
	_, err = stage.original.ReadAt(raw, 0)
	return raw, err
}
func (stage *Stage) Publish(ctx context.Context, target string) error {
	if err := stage.validate(); err != nil {
		return err
	}
	if err := Check(stage.dir, target, true); err != nil {
		return err
	}
	if existing, err := stage.dir.Lstat(target); err == nil && !existing.Mode().IsRegular() {
		return fmt.Errorf("expected a regular output file at %s", target)
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	return stage.dir.Rename(stage.name, target)
}
