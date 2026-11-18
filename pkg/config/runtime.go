package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/spf13/viper"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type RuntimeInput struct {
	CWD           string
	Env           map[string]string
	CLI           map[string]string
	UserConfigDir func() (string, error)
}

type RuntimeSettings struct {
	Root, Project, Config, Output string
	Quiet, NoColor                bool
	Verbose                       uint
	Sources                       map[string]string
	Present                       map[string]bool
}

var runtimeKinds = map[string]string{"root": "string", "project": "string", "output": "string", "quiet": "bool", "verbose": "uint", "no-color": "bool"}

type snapshotFlag struct{ name, value, kind string }

func (f snapshotFlag) HasChanged() bool    { return true }
func (f snapshotFlag) Name() string        { return f.name }
func (f snapshotFlag) ValueString() string { return f.value }
func (f snapshotFlag) ValueType() string   { return f.kind }

func runtimeValue(key, value string) (any, error) {
	switch runtimeKinds[key] {
	case "bool":
		switch strings.ToLower(value) {
		case "true", "1":
			return true, nil
		case "false", "0":
			return false, nil
		}
	case "uint":
		if value != "" && strings.Trim(value, "0123456789") == "" {
			n, err := strconv.ParseUint(value, 10, strconv.IntSize)
			if err == nil {
				return uint(n), nil
			}
		}
	case "string":
		if key == "root" || key == "project" || value != "" {
			if key != "output" || value == "text" || value == "json" {
				return value, nil
			}
		}
	}
	return nil, output.Usage("invalid runtime value for "+key+": "+strconv.Quote(value), "")
}

func envKey(key string) string { return "MF_" + strings.ToUpper(strings.ReplaceAll(key, "-", "_")) }

func ResolveRuntime(in RuntimeInput) (RuntimeSettings, error) {
	in.Env = cloneRuntimeValues(in.Env)
	in.CLI = cloneRuntimeValues(in.CLI)
	s := RuntimeSettings{Output: "text", Sources: map[string]string{}, Present: map[string]bool{}}
	fail := func(err error) (RuntimeSettings, error) { return s, err }
	// The error envelope can use a valid selected format even when bootstrap fails.
	s.Output = knownRuntimeFormat(in.CLI, in.Env, nil)
	config, explicit := in.CLI["config"]
	source := "cli"
	if !explicit {
		config, explicit = in.Env["MF_CONFIG"]
		source = "env"
	}
	if explicit {
		if config == "" {
			return fail(output.Usage("runtime config path must not be empty", ""))
		}
		s.Present["config"] = true
		s.Sources["config"] = source
	} else {
		directory := in.UserConfigDir
		if directory == nil {
			directory = os.UserConfigDir
		}
		dir, err := directory()
		if err != nil {
			return fail(output.Usage("runtime config directory: "+err.Error(), ""))
		}
		config = filepath.Join(dir, "mf", "config.toml")
		s.Sources["config"] = "default"
	}
	if !filepath.IsAbs(config) {
		config = filepath.Join(in.CWD, config)
	}
	s.Config = filepath.Clean(config)
	fileValues := map[string]any{}
	raw, err := os.ReadFile(s.Config)
	if err != nil {
		if explicit || !os.IsNotExist(err) {
			return fail(output.Usage("read runtime config "+s.Config+": "+err.Error(), ""))
		}
	} else {
		decoder, err := viper.NewCodecRegistry().Decoder("toml")
		if err != nil {
			return fail(output.Usage(err.Error(), ""))
		}
		if err := decoder.Decode(raw, fileValues); err != nil {
			return fail(output.Usage("invalid runtime TOML "+s.Config+": "+err.Error(), ""))
		}
		s.Output = knownRuntimeFormat(in.CLI, in.Env, fileValues)
		for key, value := range fileValues {
			kind, known := runtimeKinds[key]
			if !known {
				return fail(output.Usage("unknown runtime config key: "+key, ""))
			}
			valid := false
			switch kind {
			case "string":
				_, valid = value.(string)
			case "bool":
				_, valid = value.(bool)
			case "uint":
				n, ok := value.(int64)
				valid = ok && n >= 0 && uint64(n) <= uint64(^uint(0))
			}
			if !valid {
				return fail(output.Usage("invalid runtime config type for "+key, ""))
			}
			if _, err := runtimeValue(key, fmt.Sprint(value)); err != nil {
				return fail(err)
			}
		}
	}
	v := viper.New()
	for key, kind := range runtimeKinds {
		var defaultValue any = ""
		if key == "output" {
			defaultValue = "text"
		} else if kind == "bool" {
			defaultValue = false
		} else if kind == "uint" {
			defaultValue = uint(0)
		}
		v.SetDefault(key, defaultValue)
		s.Sources[key] = "default"
	}
	if err := v.MergeConfigMap(fileValues); err != nil {
		return fail(output.Usage(err.Error(), ""))
	}
	for key, kind := range runtimeKinds {
		if _, ok := fileValues[key]; ok {
			s.Sources[key] = "file"
			s.Present[key] = true
		}
		value, ok := in.CLI[key]
		origin := "cli"
		if !ok {
			value, ok = in.Env[envKey(key)]
			origin = "env"
		}
		if ok {
			validated, err := runtimeValue(key, value)
			if err != nil {
				return fail(err)
			}
			if err := v.BindFlagValue(key, snapshotFlag{key, fmt.Sprint(validated), kind}); err != nil {
				return fail(output.Usage(err.Error(), ""))
			}
			s.Sources[key] = origin
			s.Present[key] = true
		}
	}
	s.Root = v.GetString("root")
	s.Project = v.GetString("project")
	s.Output = v.GetString("output")
	s.Quiet = v.GetBool("quiet")
	s.NoColor = v.GetBool("no-color")
	s.Verbose = v.GetUint("verbose")
	if s.Root != "" && !filepath.IsAbs(s.Root) {
		base := in.CWD
		if s.Sources["root"] == "file" {
			base = filepath.Dir(s.Config)
		}
		s.Root = filepath.Join(base, s.Root)
	}
	if s.Quiet && s.Verbose > 0 {
		return fail(output.Usage("'--verbose' cannot be used with '--quiet'", ""))
	}
	return s, nil
}

func cloneRuntimeValues(values map[string]string) map[string]string {
	result := make(map[string]string, len(values))
	for k, v := range values {
		result[k] = v
	}
	return result
}

func knownRuntimeFormat(cli, env map[string]string, file map[string]any) string {
	for _, value := range []string{cli["output"], env["MF_OUTPUT"], fmt.Sprint(file["output"])} {
		if v, e := runtimeValue("output", value); e == nil {
			return v.(string)
		}
	}
	return "text"
}
