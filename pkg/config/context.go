package config

import (
	"fmt"
	"os"
	"path/filepath"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type ContextOptions struct {
	Runtime *RuntimeSettings
	ResolveOptions
	Project string
	Format  string
	JSON    bool
	NoColor bool
	Quiet   bool
	Verbose uint
}

type Context struct {
	Format     string
	ConfigPath string
	RepoRoot   string
	CWD        string
	Color      bool
	Quiet      bool
	project    string
}

func NewContext(options ContextOptions) (*Context, error) {
	if s := options.Runtime; s != nil {
		options.Root = s.Root
		options.Config = s.Config
		options.Project = s.Project
		options.Format = s.Output
		options.Quiet = s.Quiet
		options.NoColor = s.NoColor
		options.Verbose = s.Verbose
	}
	if options.Quiet && options.Verbose != 0 {
		return nil, output.Usage("'--verbose' cannot be used with '--quiet'", "")
	}
	configPath := options.Config
	if configPath == "" {
		directory, err := os.UserConfigDir()
		if err != nil {
			return nil, output.Usage("runtime config directory: "+err.Error(), "")
		}
		configPath = filepath.Join(directory, "mf", "config.toml")
	}
	if options.CWD == "" {
		cwd, err := os.Getwd()
		if err != nil {
			return nil, err
		}
		options.CWD = cwd
	}
	resolve := options.ResolveOptions
	resolve.Config = ""
	root, err := ResolveRoot(resolve)
	if err != nil {
		return nil, err
	}
	format := options.Format
	if format == "" {
		format = "text"
	}
	if options.JSON {
		format = "json"
	}
	return &Context{Format: format, ConfigPath: configPath, RepoRoot: root, CWD: options.CWD, Color: !options.NoColor, Quiet: options.Quiet, project: options.Project}, nil
}

func DefaultConfigPath(platform, homeDirectory, xdgConfigHome string) (string, error) {
	if homeDirectory == "" {
		return "", fmt.Errorf("unable to determine platform configuration directory for mf")
	}
	var directory string
	switch platform {
	case "darwin":
		directory = filepath.Join(homeDirectory, "Library", "Application Support")
	case "linux":
		directory = filepath.Join(homeDirectory, ".config")
		if xdgConfigHome != "" && !filepath.IsAbs(xdgConfigHome) {
			return "", fmt.Errorf("XDG_CONFIG_HOME must be absolute")
		}
		if filepath.IsAbs(xdgConfigHome) {
			directory = xdgConfigHome
		}
	default:
		return "", fmt.Errorf("unsupported platform %q", platform)
	}
	return filepath.Join(directory, "mf", "config.toml"), nil
}

func (context *Context) Project() string {
	if context.RepoRoot == "" {
		return ""
	}
	return context.project
}

func (context *Context) RequireRepo() (string, error) {
	if context.RepoRoot == "" {
		return "", output.NotInRepo()
	}
	return context.RepoRoot, nil
}
