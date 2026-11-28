package report

import (
	"context"
	"errors"
	"maps"
	"os"
	"path/filepath"
	"slices"
)

func Scaffold(ctx context.Context, root *os.Root, dir string, files map[string][]byte) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := Check(root, dir, true); err != nil {
		return err
	}
	if err := root.MkdirAll(filepath.Dir(dir), 0755); err != nil {
		return err
	}
	if err := root.Mkdir(dir, 0755); err != nil {
		return err
	}
	success := false
	defer func() {
		if !success {
			_ = root.RemoveAll(dir)
		}
	}()

	target, err := root.OpenRoot(dir)
	if err != nil {
		return err
	}
	defer target.Close()
	write := func(name string, data []byte) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		file, err := target.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
		if err != nil {
			return err
		}
		_, err = file.Write(data)
		return errors.Join(err, file.Close())
	}
	for _, name := range slices.Sorted(maps.Keys(files)) {
		if name != "report.yaml" {
			if err := write(name, files[name]); err != nil {
				return err
			}
		}
	}
	const manifestStage = ".report.yaml.tmp"
	if err := write(manifestStage, files["report.yaml"]); err != nil {
		return err
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := target.Rename(manifestStage, "report.yaml"); err != nil {
		return err
	}
	success = true
	return nil
}
