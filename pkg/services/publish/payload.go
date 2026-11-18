package publish

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/markdown"
	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/services/content"
)

func configMap(target model.PublishTarget) map[string]any {
	raw, err := json.Marshal(target.Config)
	if err != nil {
		return nil
	}
	var result map[string]any
	_ = json.Unmarshal(raw, &result)
	return result
}
func configString(target model.PublishTarget, key string) string {
	value, _ := configMap(target)[key].(string)
	return value
}
func joined(root, path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(root, path)
}
func readText(path string) (string, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if !utf8.Valid(raw) {
		return "", fmt.Errorf("%s: stream did not contain valid UTF-8", path)
	}
	return string(raw), nil
}
func problem(kind, message, hint string) *output.Error {
	e := &output.Error{Kind: kind, Message: message}
	if hint != "" {
		e.Hint = &hint
	}
	return e
}

func LocalPath(base string, target model.PublishTarget, article model.Article) (string, *model.EffectiveDateOut, error) {
	path := ""
	if target.Path != nil {
		path = *target.Path
	} else {
		var ok bool
		path, ok = configMap(target)["path"].(string)
		if !ok {
			message := "local target '" + target.Name + "' missing config.path"
			if target.Config == nil {
				message = "local target '" + target.Name + "' missing path"
			}
			return "", nil, output.Usage(message, "set `path: <directory>` or `config.path: <directory>` on the target")
		}
	}
	template, err := content.ParsePathTemplate(path)
	if err != nil {
		return "", nil, err
	}
	date := time.Now()
	var info *model.EffectiveDateOut
	if template.MostSpecificFormat() != "" {
		origin := "filename_prefix"
		valid := false
		if article.TemplateOrigin != nil {
			date, err = time.Parse("2006-01-02", article.TemplateOrigin.SlotValue)
			valid = err == nil
			if valid {
				origin = "template_slot"
			}
		}
		if !valid {
			date, valid = content.LeadingFilenameDate(filepath.Base(article.Path))
		}
		if !valid {
			return "", nil, problem("no_effective_date", "article has no effective date", "add a YYYY-MM-DD prefix to the filename, e.g. 'docs/blog/2026-05-15-launch.md'")
		}
		info = &model.EffectiveDateOut{Date: date.Format("2006-01-02"), Origin: origin}
	}
	return joined(base, template.Expand(date)), info, nil
}

func PrepareContent(raw string, config *model.MindConfig, article model.Article, strip, keep bool) string {
	if article.TemplateOrigin != nil {
		raw = markdown.StripMindForgeFrontMatter(markdown.StripTyporaFrontMatter(raw))
	}
	stripAll := config.Publish.StripFrontMatter != nil && *config.Publish.StripFrontMatter
	if strip {
		stripAll = true
	} else if keep {
		stripAll = false
	}
	if stripAll {
		_, body, eol, ok := markdown.InitialFrontMatter(raw)
		if ok {
			raw = strings.TrimPrefix(body, eol)
		}
	}
	return raw
}

func Transforms(raw, artifactDir string, target model.PublishTarget, project string) (string, model.PayloadTransforms, error) {
	report := model.PayloadTransforms{SVGPNGReplaced: []string{}, SVGPNGMissing: []string{}}
	if enabled, _ := configMap(target)["svg_to_png"].(bool); enabled {
		raw = markdown.RewriteReferences(raw, func(reference string) (string, bool) {
			if !strings.HasSuffix(reference, ".svg") || !markdown.ShouldRewriteTarget(reference) {
				return "", false
			}
			png := strings.TrimSuffix(reference, ".svg") + ".png"
			if _, err := os.Stat(joined(artifactDir, png)); err == nil {
				report.SVGPNGReplaced = append(report.SVGPNGReplaced, reference)
				return filepath.ToSlash(png), true
			}
			report.SVGPNGMissing = append(report.SVGPNGMissing, reference)
			return "", false
		})
	}
	banner := configString(target, "banner_markdown")
	if strings.TrimSpace(banner) == "" {
		path := configString(target, "banner_file")
		if strings.TrimSpace(path) != "" {
			var err error
			banner, err = readText(joined(project, path))
			if err != nil {
				return "", report, output.Usage(fmt.Sprintf("banner_file '%s' could not be read: %v", path, err), "check publish.targets[].config.banner_file path (project-relative)")
			}
		}
	}
	if banner != "" {
		banner = strings.TrimRight(banner, " \t\r\n")
		if !strings.HasPrefix(raw, banner) {
			raw = banner + "\n\n" + raw
			report.BannerInjected = true
		}
	}
	return raw, report, nil
}

func Upsert(existing *model.PublishRecord, path, target string, status, url *string, now time.Time) (model.PublishRecord, string, error) {
	if existing != nil {
		record := *existing
		if status != nil {
			record.Status = *status
		}
		if url != nil {
			record.TargetURL = url
		}
		return record, "updated", nil
	}
	if status == nil {
		return model.PublishRecord{}, "", output.Usage("cannot create record without --status", "provide --status draft|published|archived")
	}
	record := model.PublishRecord{Path: path, TargetName: target, Status: *status, TargetURL: url}
	if *status == "archived" && url == nil {
		return record, "", output.Usage("cannot create initial 'archived' record without --target-url", "provide --target-url <URL>, or first record with --status published and then --status archived")
	}
	if *status == "published" || *status == "archived" {
		timestamp := now.UTC().Format(time.RFC3339)
		record.PublishedAt = &timestamp
	}
	return record, "created", nil
}
