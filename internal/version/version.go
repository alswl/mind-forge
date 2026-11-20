// Package version describes the executable without invoking build tools at runtime.
package version

import (
	"fmt"
	"runtime"
	"runtime/debug"
	"strings"
)

// make build sets these with -ldflags. A plain Go build has no release stamp.
var (
	Version   = "v0.0.0"
	Commit    = "unknown"
	BuildDate = "unknown"
)

type Metadata struct {
	Version      string `json:"version"`
	BaseVersion  string `json:"base_version"`
	Channel      string `json:"channel"`
	Commit       string `json:"commit"`
	BuildDate    string `json:"build_date"`
	Rustc        string `json:"rustc"`
	Compiler     string `json:"compiler"`
	GoVersion    string `json:"go_version"`
	TargetTriple string `json:"target_triple"`
}

func New(commit, buildDate, compiler, goVersion, arch, operatingSystem string) Metadata {
	if commit == "" {
		commit = "unknown"
	}
	if buildDate == "" {
		buildDate = "unknown"
	}
	switch arch {
	case "amd64":
		arch = "x86_64"
	case "arm64":
		arch = "aarch64"
	}
	if operatingSystem == "darwin" {
		operatingSystem = "macos"
	}
	base := strings.TrimPrefix(Version, "v")
	if suffix := strings.IndexAny(base, "-+"); suffix >= 0 {
		base = base[:suffix]
	}
	return Metadata{
		Version: base + "-dev+" + commit, BaseVersion: base, Channel: "dev",
		Commit: commit, BuildDate: buildDate, Rustc: "not applicable",
		Compiler: compiler, GoVersion: goVersion, TargetTriple: arch + "-" + operatingSystem,
	}
}

func Current() Metadata {
	commit := Commit
	if commit == "unknown" || commit == "" {
		if info, ok := debug.ReadBuildInfo(); ok {
			for _, setting := range info.Settings {
				if setting.Key == "vcs.revision" && setting.Value != "" {
					commit = setting.Value
					if len(commit) > 7 {
						commit = commit[:7]
					}
				}
			}
		}
	}
	return New(commit, BuildDate, runtime.Compiler, runtime.Version(), runtime.GOARCH, runtime.GOOS)
}

func (metadata Metadata) Text() string {
	return fmt.Sprintf("mf %s (built %s, %s %s)\n", metadata.Version, metadata.BuildDate, metadata.Compiler, metadata.GoVersion)
}
