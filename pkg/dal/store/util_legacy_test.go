package store

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestLegacySchemaValidation(t *testing.T) {
	t.Run("test_validate_schema_version_1_is_compatible", func(t *testing.T) {
		if err := ValidateSchema("1", filepath.Join(t.TempDir(), "test.yaml")); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("test_validate_schema_version_unknown_is_incompatible", func(t *testing.T) {
		err := ValidateSchema("2", filepath.Join(t.TempDir(), "test.yaml"))
		var typed *output.Error
		if !errors.As(err, &typed) || typed.Kind != "incompatible_schema" || typed.ExitCode() != 2 {
			t.Fatalf("incompatible schema error = %v", err)
		}
	})
}

func TestLegacyCanonicalWithin(t *testing.T) {
	for _, name := range []string{"test_canonicalize_within_existing_target", "test_canonicalize_within_non_existent_leaf", "test_canonicalize_within_escape_via_dotdot", "test_canonicalize_within_symlink_escape", "test_canonicalize_within_root_not_found", "test_canonicalize_within_invalid_leaf_name"} {
		t.Run(name, func(t *testing.T) {
			parent := t.TempDir()
			root := filepath.Join(parent, "repo")
			if err := os.Mkdir(root, 0o755); err != nil {
				t.Fatal(err)
			}
			target := filepath.Join(root, "new-project")
			switch name {
			case "test_canonicalize_within_existing_target":
				target = filepath.Join(root, "subdir")
				if err := os.Mkdir(target, 0o755); err != nil {
					t.Fatal(err)
				}
			case "test_canonicalize_within_escape_via_dotdot":
				target = filepath.Join(root, "../outside")
			case "test_canonicalize_within_symlink_escape":
				outside := filepath.Join(parent, "escape")
				if err := os.Mkdir(outside, 0o755); err != nil {
					t.Fatal(err)
				}
				target = filepath.Join(root, "evil-link")
				if err := os.Symlink(outside, target); err != nil {
					t.Fatal(err)
				}
			case "test_canonicalize_within_root_not_found":
				root = filepath.Join(parent, "nonexistent")
				target = filepath.Join(parent, "somewhere")
			case "test_canonicalize_within_invalid_leaf_name":
				target = filepath.Join(root, "../escape")
			}
			got, err := CanonicalWithin(root, target)
			switch name {
			case "test_canonicalize_within_existing_target", "test_canonicalize_within_non_existent_leaf":
				canonical, resolveErr := filepath.EvalSymlinks(root)
				if resolveErr != nil {
					t.Fatal(resolveErr)
				}
				if err != nil || !strings.HasPrefix(got, canonical) {
					t.Fatalf("safe canonical path = %q, %v", got, err)
				}
			case "test_canonicalize_within_root_not_found":
				var pathError *os.PathError
				if !errors.As(err, &pathError) {
					t.Fatalf("missing root did not return IO error: %v", err)
				}
			case "test_canonicalize_within_invalid_leaf_name":
				if err == nil || !(strings.Contains(err.Error(), "invalid") || strings.Contains(err.Error(), "outside")) {
					t.Fatalf("invalid leaf accepted or misreported: %q %v", got, err)
				}
			case "test_canonicalize_within_escape_via_dotdot":
				if err == nil || !strings.Contains(err.Error(), "outside") {
					t.Fatalf("traversal accepted or misreported: %q %v", got, err)
				}
			default:
				if err == nil {
					t.Fatalf("symlink escape accepted: %q", got)
				}
			}
		})
	}
}
