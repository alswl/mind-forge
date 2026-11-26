package build

import (
	"context"
	"fmt"
	"os"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func abandonedArticle(path string) error {
	return output.Usage(fmt.Sprintf("cannot build abandoned article %q", path), "restore it with `mf article update <ARTICLE_PATH> --status draft --project <PROJECT>`")
}

type abandonedFile struct {
	path string
	info os.FileInfo
}

func abandonedArticleFiles(ctx context.Context, project string, articles []model.Article) ([]abandonedFile, error) {
	var files []abandonedFile
	for _, article := range articles {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		if article.Status != model.ArticleStatusAbandoned {
			continue
		}
		if info, err := os.Stat(joined(project, article.Path)); err == nil {
			files = append(files, abandonedFile{path: article.Path, info: info})
		}
	}
	return files, nil
}

func checkAbandonedArticle(info os.FileInfo, files []abandonedFile) error {
	for _, file := range files {
		if os.SameFile(info, file.info) {
			return abandonedArticle(file.path)
		}
	}
	return nil
}
