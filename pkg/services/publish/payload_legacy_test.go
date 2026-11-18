package publish

import (
	"errors"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

// Frozen payload assertions are component tests: temporary sibling files stand
// in for assets, with no CLI, network, process, or publication workflow.
func TestPublishTransformsFrozenAssertions(t *testing.T) {
	target := func(config map[string]any) model.PublishTarget {
		return model.PublishTarget{Name: "test-target", Type: "yuque-prompt", Enabled: true, Config: config}
	}
	t.Run("svg_to_png_transform_rewrites_markdown_and_html_and_records_replacements", func(t *testing.T) {
		root := t.TempDir()
		putPublish(t, root, "assets/logo.png", "png")
		input := "# Hello\n\n![logo](assets/logo.svg)\n\n<img src=\"assets/logo.svg\" alt=\"logo\">\n"
		body, report, err := Transforms(input, root, target(map[string]any{"svg_to_png": true}), root)
		if err != nil || !strings.Contains(body, "![logo](assets/logo.png)") || !strings.Contains(body, `<img src="assets/logo.png" alt="logo">`) || strings.Contains(body, "assets/logo.svg") || !reflect.DeepEqual([]string(report.SVGPNGReplaced), []string{"assets/logo.svg", "assets/logo.svg"}) || len(report.SVGPNGMissing) != 0 {
			t.Fatalf("%s %+v %v", body, report, err)
		}
	})
	t.Run("svg_to_png_transform_keeps_missing_and_skips_fenced_absolute_url_and_data_refs", func(t *testing.T) {
		root := t.TempDir()
		input := "```markdown\n![fenced](assets/logo.svg)\n```\n\n![missing](assets/missing.svg)\n![absolute](/assets/absolute.svg)\n![url](https://example.test/logo.svg)\n![data](data:image/svg+xml;base64,AAAA)\n"
		body, report, err := Transforms(input, root, target(map[string]any{"svg_to_png": true}), root)
		if err != nil || body != input || len(report.SVGPNGReplaced) != 0 || !reflect.DeepEqual([]string(report.SVGPNGMissing), []string{"assets/missing.svg"}) {
			t.Fatalf("%s %+v %v", body, report, err)
		}
	})
	for _, test := range []struct {
		name                    string
		config                  map[string]any
		input, prefix, contains string
		replaced, missing       []string
		banner                  bool
	}{
		{"publish_transforms_applies_svg_to_png_when_enabled", map[string]any{"svg_to_png": true}, "![logo](assets/logo.svg)\n", "", "![logo](assets/logo.png)", []string{"assets/logo.svg"}, nil, false},
		{"publish_transforms_skips_svg_to_png_when_not_enabled", map[string]any{}, "![logo](assets/logo.svg)\n", "", "![logo](assets/logo.svg)", nil, nil, false},
		{"publish_transforms_injects_banner_when_configured", map[string]any{"banner_markdown": ":::warning\nHeader\n:::"}, "# Body\n", ":::warning\nHeader\n:::\n\n", "# Body", nil, nil, true},
		{"publish_transforms_banner_idempotent", map[string]any{"banner_markdown": ":::warning\nHeader\n:::"}, ":::warning\nHeader\n:::\n\n# Body\n", ":::warning\nHeader\n:::\n\n", "# Body", nil, nil, false},
		{"publish_transforms_applies_both_when_configured", map[string]any{"svg_to_png": true, "banner_markdown": ":::warning\nBanner\n:::"}, "![logo](assets/logo.svg)\n", ":::warning\nBanner\n:::\n\n", "assets/logo.png", []string{"assets/logo.svg"}, nil, true},
		{"publish_transforms_records_missing_png", map[string]any{"svg_to_png": true}, "![missing](assets/no_png.svg)\n", "", "![missing](assets/no_png.svg)", nil, []string{"assets/no_png.svg"}, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			putPublish(t, root, "assets/logo.png", "png")
			body, report, err := Transforms(test.input, root, target(test.config), root)
			if err != nil || !strings.HasPrefix(body, test.prefix) || !strings.Contains(body, test.contains) || report.BannerInjected != test.banner {
				t.Fatalf("%s %+v %v", body, report, err)
			}
			if strings.Contains(test.name, "idempotent") && body != test.input {
				t.Fatal("banner was duplicated")
			}
			if len(report.SVGPNGReplaced) != len(test.replaced) || len(report.SVGPNGMissing) != len(test.missing) {
				t.Fatalf("%+v", report)
			}
			for n, value := range test.replaced {
				if report.SVGPNGReplaced[n] != value {
					t.Fatal(report)
				}
			}
			for n, value := range test.missing {
				if report.SVGPNGMissing[n] != value {
					t.Fatal(report)
				}
			}
		})
	}
	t.Run("publish_transforms_banner_file_unreadable_is_usage_error", func(t *testing.T) {
		root := t.TempDir()
		_, _, err := Transforms("body", root, target(map[string]any{"banner_file": "nonexistent.md"}), root)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "usage" || !strings.Contains(err.Error(), "banner_file") {
			t.Fatalf("%v", err)
		}
	})
}

func TestLegacyPublishErrors(t *testing.T) {
	t.Run("no_effective_date_kind_and_hint", func(t *testing.T) {
		path := "archive/{date:YYYY-MM-DD}"
		_, _, err := LocalPath(t.TempDir(), model.PublishTarget{Path: &path}, model.Article{Path: "docs/test.md"})
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "no_effective_date" || failure.Hint == nil || !strings.Contains(*failure.Hint, "YYYY-MM-DD") {
			t.Fatalf("missing date = %v", err)
		}
	})
	t.Run("no_article_files_kind_and_hint", func(t *testing.T) {
		config := model.DefaultMindConfig()
		_, _, err := LocateArtifact(t.TempDir(), &config, model.Article{Title: "test", Path: "docs/test.md"}, nil)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "no_article_files" || failure.Hint == nil || !strings.Contains(*failure.Hint, "article file") {
			t.Fatalf("missing article = %v", err)
		}
	})
}
