package report

import (
	"compress/gzip"
	"embed"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
)

//go:embed engine/*.cjs.gz
var engineFiles embed.FS

func embeddedEngine() (node, directory string, err error) {
	node, err = exec.LookPath("node")
	if err != nil {
		return "", "", fmt.Errorf("Node 24 is required for report builds: node executable not found on PATH: %w", err)
	}
	directory, err = os.MkdirTemp("", "mf-report-engine-")
	if err != nil {
		return "", "", err
	}
	for _, name := range []string{"worker.cjs", "builtins.cjs"} {
		if err := expandEngineFile(directory, name); err != nil {
			_ = os.RemoveAll(directory)
			return "", "", fmt.Errorf("prepare embedded report engine: %w", err)
		}
	}
	return node, directory, nil
}

func expandEngineFile(directory, name string) error {
	source, err := engineFiles.Open("engine/" + name + ".gz")
	if err != nil {
		return err
	}
	defer source.Close()
	compressed, err := gzip.NewReader(source)
	if err != nil {
		return err
	}
	defer compressed.Close()
	target, err := os.OpenFile(filepath.Join(directory, name), os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return err
	}
	_, copyError := io.Copy(target, compressed)
	return errors.Join(copyError, target.Close())
}
