package config

import _ "embed"

// These published schemas preserve constraints and defaults independently of
// reflection rules in the implementation language. Update both representations
// when the configuration contract changes.
//
//go:embed schema/mind-config.json
var configSchemaJSON string

//go:embed schema/mind-config.yaml
var configSchemaYAML string

func ConfigSchema(format string) string {
	if format == "yaml" {
		return configSchemaYAML
	}
	return configSchemaJSON
}
