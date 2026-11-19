package config

import (
	"errors"
	"path/filepath"
)

// snapshotUserConfigDir follows Go 1.26.11 os.UserConfigDir rules using the
// invocation's frozen environment. The stdlib API reads the live environment
// and cannot accept a lookup, so call it neither later nor via temporary Setenv.
func snapshotUserConfigDir(platform string, env map[string]string) (string, error) {
	switch platform {
	case "windows":
		if dir := env["AppData"]; dir != "" {
			return dir, nil
		}
		return "", errors.New("%AppData% is not defined")
	case "darwin", "ios":
		if dir := env["HOME"]; dir != "" {
			return dir + "/Library/Application Support", nil
		}
		return "", errors.New("$HOME is not defined")
	case "plan9":
		if dir := env["home"]; dir != "" {
			return dir + "/lib", nil
		}
		return "", errors.New("$home is not defined")
	default:
		if dir := env["XDG_CONFIG_HOME"]; dir != "" {
			if !filepath.IsAbs(dir) {
				return "", errors.New("path in $XDG_CONFIG_HOME is relative")
			}
			return dir, nil
		}
		if dir := env["HOME"]; dir != "" {
			return dir + "/.config", nil
		}
		return "", errors.New("neither $XDG_CONFIG_HOME nor $HOME are defined")
	}
}
