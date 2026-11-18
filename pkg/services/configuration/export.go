package configuration

import (
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type Exporter struct{ Write func(string, []byte) error }

func (service Exporter) Export(cwd, root, format, path string) error {
	config, err := app.EffectiveConfig(cwd, root)
	if err != nil {
		return err
	}
	content, err := app.SerializeConfig(config, format)
	if err != nil {
		return err
	}
	write := service.Write
	if write == nil {
		write = store.ExportFile
	}
	return write(path, []byte(content))
}
