package store

import (
	"context"
	"os"
	"path/filepath"
)

// MarkdownSourceFiles lists an authored document's inputs in assembly order.
func MarkdownSourceFiles(ctx context.Context, source string, directory bool) ([]string, error) {
	if !directory {
		return []string{source}, nil
	}
	entries, err := os.ReadDir(source)
	if err != nil {
		return nil, err
	}
	var files []string
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		if filepath.Ext(entry.Name()) == ".md" {
			files = append(files, filepath.Join(source, entry.Name()))
		}
	}
	return files, nil
}
