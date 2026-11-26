package versioncheck

import (
	"cmp"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"
	"time"
)

const (
	latestReleaseURL = "https://api.github.com/repos/alswl/mind-forge/releases/latest"
	InstallURL       = "https://github.com/alswl/mind-forge/blob/master/docs/guide/install.md"
	maxReleaseBody   = 1 << 20
	checkTimeout     = 5 * time.Second
)

type Status string

const (
	UpdateAvailable Status = "update_available"
	UpToDate        Status = "up_to_date"
	Ahead           Status = "ahead"
)

type Result struct {
	CurrentVersion string `json:"current_version"`
	LatestVersion  string `json:"latest_version"`
	Status         Status `json:"status"`
	UpgradeURL     string `json:"upgrade_url,omitempty"`
}

func Check(ctx context.Context, current string) (Result, error) {
	return check(ctx, current, nil)
}

func check(ctx context.Context, current string, transport http.RoundTripper) (Result, error) {
	local, ok := parseSemver(current)
	if !ok {
		return Result{}, fmt.Errorf("invalid local version %q", current)
	}
	ctx, cancel := context.WithTimeout(ctx, checkTimeout)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, latestReleaseURL, nil)
	if err != nil {
		return Result{}, err
	}
	request.Header.Set("Accept", "application/vnd.github+json")
	client := http.Client{
		Transport: transport,
		CheckRedirect: func(*http.Request, []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
	response, err := client.Do(request)
	if err != nil {
		return Result{}, fmt.Errorf("requesting the latest release from GitHub: %w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return Result{}, fmt.Errorf("GitHub latest release request returned HTTP %d", response.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, maxReleaseBody+1))
	if err != nil {
		return Result{}, fmt.Errorf("reading the GitHub release response: %w", err)
	}
	if len(body) > maxReleaseBody {
		return Result{}, fmt.Errorf("GitHub release response exceeds %d bytes", maxReleaseBody)
	}
	var release struct {
		Tag        string `json:"tag_name"`
		Draft      *bool  `json:"draft"`
		Prerelease *bool  `json:"prerelease"`
	}
	if err := json.Unmarshal(body, &release); err != nil {
		return Result{}, fmt.Errorf("invalid GitHub release JSON: %w", err)
	}
	latest, tagged := strings.CutPrefix(release.Tag, "v")
	stable, valid := parseSemver(latest)
	if !tagged || !valid || stable.prerelease || release.Draft == nil || *release.Draft ||
		release.Prerelease == nil || *release.Prerelease {
		return Result{}, fmt.Errorf("GitHub response does not identify a published stable SemVer release")
	}

	result := Result{CurrentVersion: current, LatestVersion: latest, Status: UpToDate}
	switch local.compareStable(stable) {
	case -1:
		result.Status = UpdateAvailable
		result.UpgradeURL = InstallURL
	case 1:
		result.Status = Ahead
	}
	return result, nil
}

type semver struct {
	core       [3]string
	prerelease bool
}

var semverPattern = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$`)

func parseSemver(value string) (semver, bool) {
	parts := semverPattern.FindStringSubmatch(value)
	if parts == nil {
		return semver{}, false
	}
	for id := range strings.SplitSeq(parts[4], ".") {
		if len(id) > 1 && id[0] == '0' && strings.Trim(id, "0123456789") == "" {
			return semver{}, false
		}
	}
	return semver{core: [3]string{parts[1], parts[2], parts[3]}, prerelease: parts[4] != ""}, true
}

func (local semver) compareStable(stable semver) int {
	for i, component := range local.core {
		other := stable.core[i]
		// Valid core numbers have no leading zeroes and can exceed Go integer sizes.
		if order := cmp.Compare(len(component), len(other)); order != 0 {
			return order
		}
		if order := cmp.Compare(component, other); order != 0 {
			return order
		}
	}
	if local.prerelease {
		return -1
	}
	return 0
}
