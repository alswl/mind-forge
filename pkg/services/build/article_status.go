package build

import (
	"fmt"
	"os"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func abandonedArticle(path string) error {
	return output.Usage(fmt.Sprintf("cannot build abandoned article %q", path), "use `mf article update --status` to restore the article before building")
}

func checkLegacyBuildStatus(project, source, configuredFile string, articles []model.Article) error {
	sourceInfo, _ := os.Stat(source)
	var configuredInfo os.FileInfo
	if configuredFile != "" && configuredFile != source {
		configuredInfo, _ = os.Stat(configuredFile)
	}
	for _, article := range articles {
		if article.Status != model.ArticleStatusAbandoned {
			continue
		}
		registered, err := os.Stat(joined(project, article.Path))
		if err != nil {
			continue
		}
		if sourceInfo != nil && os.SameFile(sourceInfo, registered) ||
			configuredInfo != nil && os.SameFile(configuredInfo, registered) {
			return abandonedArticle(article.Path)
		}
	}
	return nil
}
