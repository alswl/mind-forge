//go:build mf_testfaults

package lifecycle

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"

	manager "github.com/alswl/mind-forge/pkg/managers/lifecycle"
)

const lifecycleTestFaultEnv = "MF_TEST_LIFECYCLE_PREPARE_FAILURE"

func newEngine() manager.Engine {
	stage := os.Getenv(lifecycleTestFaultEnv)
	engine := manager.Engine{}
	if stage == "partial" {
		engine.Filesystem = partialJournalFilesystem{}
		return engine
	}
	if stage != "" {
		engine.Checkpoint = func(name string) error {
			if name == "prepare:"+stage {
				return fmt.Errorf("injected_failure:%s", stage)
			}
			return nil
		}
	}
	return engine
}

type partialJournalFilesystem struct{ manager.OSFilesystem }

func (fs partialJournalFilesystem) OpenFile(path string, flags int, mode os.FileMode) (manager.DurableFile, error) {
	file, err := fs.OSFilesystem.OpenFile(path, flags, mode)
	if err != nil {
		return nil, err
	}
	if filepath.Base(path) == "journal.json.next" {
		return &partialJournalFile{DurableFile: file}, nil
	}
	return file, nil
}

type partialJournalFile struct {
	manager.DurableFile
	failed bool
}

func (file *partialJournalFile) Write(data []byte) (int, error) {
	if !file.failed {
		file.failed = true
		n, err := file.DurableFile.Write(data[:len(data)/2])
		return n, errors.Join(err, errors.New("injected_failure:partial"))
	}
	return file.DurableFile.Write(data)
}

var _ io.Writer = (*partialJournalFile)(nil)
