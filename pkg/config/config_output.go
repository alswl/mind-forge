package config

import (
	"bytes"
	"encoding/json"

	"github.com/alswl/mind-forge/pkg/common/model"
	"go.yaml.in/yaml/v3"
)

func EffectiveConfig(cwd, repoRoot string) (model.MindConfig, error) {
	config := model.DefaultMindConfig()
	layer, err := LoadProjectConfig(cwd, repoRoot)
	if err != nil {
		return config, err
	}
	if layer != nil {
		config = MergeConfig(config, *layer)
	}
	_, err = ResolveAndValidateLayout(canonical(cwd), &config)
	return config, err
}

func SerializeConfig(config model.MindConfig, format string) (string, error) {
	var buffer bytes.Buffer
	if format == "json" {
		encoder := json.NewEncoder(&buffer)
		encoder.SetEscapeHTML(false)
		if err := encoder.Encode(config); err != nil {
			return "", err
		}
		return string(bytes.TrimSuffix(buffer.Bytes(), []byte{'\n'})), nil
	}
	encoder := yaml.NewEncoder(&buffer)
	encoder.SetIndent(2)
	encoder.CompactSeqIndent()
	if err := encoder.Encode(config); err != nil {
		return "", err
	}
	if err := encoder.Close(); err != nil {
		return "", err
	}
	return buffer.String(), nil
}
