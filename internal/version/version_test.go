package version

import (
	"encoding/json"
	"runtime"
	"strings"
	"testing"
)

func TestMetadataPreservesVersionContract(t *testing.T) {
	original := Version
	t.Cleanup(func() { Version = original })
	Version = "v1.2.3-rc.1+build.2"
	info := New("abc1234", "2026-11-16", "gc", "go1.26.11", "arm64", "darwin")
	raw, err := json.Marshal(info)
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]string
	if err := json.Unmarshal(raw, &fields); err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"version": "1.2.3-dev+abc1234", "base_version": "1.2.3", "channel": "dev",
		"commit": "abc1234", "build_date": "2026-11-16", "rustc": "not applicable",
		"compiler": "gc", "go_version": "go1.26.11", "target_triple": "aarch64-macos",
	}
	if len(fields) != len(want) {
		t.Fatalf("unexpected metadata keys: %s", raw)
	}
	for key, value := range want {
		if fields[key] != value {
			t.Errorf("%s = %q, want %q", key, fields[key], value)
		}
	}
	if got := info.Text(); got != "mf 1.2.3-dev+abc1234 (built 2026-11-16, gc go1.26.11)\n" {
		t.Fatalf("version text: %q", got)
	}
}

func TestMetadataRuntimePlatformNames(t *testing.T) {
	for _, test := range []struct{ arch, os, want string }{
		{"amd64", "darwin", "x86_64-macos"}, {"arm64", "darwin", "aarch64-macos"},
		{"amd64", "linux", "x86_64-linux"}, {"arm64", "linux", "aarch64-linux"},
	} {
		if got := New("", "", "gc", "go1.26.11", test.arch, test.os); got.TargetTriple != test.want || got.Commit != "unknown" || got.BuildDate != "unknown" {
			t.Errorf("%s/%s: %#v", test.os, test.arch, got)
		}
	}
}

func TestCurrentReportsActualCompiler(t *testing.T) {
	info := Current()
	if info.Compiler != runtime.Compiler || info.GoVersion != runtime.Version() {
		t.Fatalf("compiler metadata does not describe this executable: %#v", info)
	}
	if strings.Contains(info.Text(), "rustc") || info.Rustc != "not applicable" {
		t.Fatalf("retired compiler must not be presented as active: %#v", info)
	}
}

func TestTaggedReleaseVersion(t *testing.T) {
	originalVersion, originalRelease := Version, ReleaseBuild
	t.Cleanup(func() { Version, ReleaseBuild = originalVersion, originalRelease })
	ReleaseBuild = "true"

	for _, test := range []struct{ tag, version, channel string }{
		{"v1.2.3", "1.2.3", "release"},
		{"v1.2.3+build.4", "1.2.3+build.4", "release"},
		{"v1.2.3-rc.1+build.4", "1.2.3-rc.1+build.4", "prerelease"},
	} {
		Version = test.tag
		got := New("abc1234", "2026-11-16", "gc", "go1.26.11", "arm64", "darwin")
		if got.Version != test.version || got.Channel != test.channel || got.BaseVersion != "1.2.3" || got.Commit != "abc1234" {
			t.Errorf("%s: unexpected metadata: %#v", test.tag, got)
		}
		if !strings.HasPrefix(got.Text(), "mf "+test.version+" (") {
			t.Errorf("%s: unexpected version text: %q", test.tag, got.Text())
		}
	}
}
