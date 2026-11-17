package publish

import (
	"errors"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"regexp"
	"testing"
)

func TestOriginalErrorVariants(t *testing.T) {
	path := "archive/{date:YYYY-MM-DD}"
	_, _, date := LocalPath(t.TempDir(), model.PublishTarget{Path: &path}, model.Article{Path: "docs/x.md"})
	config := model.DefaultMindConfig()
	_, _, empty := LocateArtifact(t.TempDir(), &config, model.Article{Title: "x", Path: "docs/x.md"}, nil)
	_, _, missing := LocateArtifact(t.TempDir(), &config, model.Article{Title: "x", Path: "docs/x.md"}, &model.Output{BuildPath: "build/x.md"})
	for _, row := range []struct {
		name string
		err  error
		kind string
	}{{"NoEffectiveDate", date, "no_effective_date"}, {"NoArticleFiles", empty, "no_article_files"}, {"BuildArtifactMissing", missing, "build_artifact_missing"}} {
		t.Run(row.name, func(t *testing.T) {
			var failure *output.Error
			if !errors.As(row.err, &failure) || failure.Kind != row.kind || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(failure.Kind) {
				t.Fatal(row.err)
			}
			if failure.Hint == nil {
				t.Fatalf("hint absent for %s", row.name)
			}
		})
	}
}
