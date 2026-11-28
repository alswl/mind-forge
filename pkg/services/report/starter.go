package report

import (
	"embed"
	"fmt"
	"io/fs"
)

//go:embed starter/*
var starterFiles embed.FS

type ScaffoldResult struct {
	Directory string   `json:"directory"`
	Renderer  string   `json:"renderer"`
	Install   []string `json:"install"`
}

func starter(item Item) (map[string][]byte, error) {
	raw := item.ExampleData
	if len(raw) == 0 {
		raw = []byte("{}")
	}
	files := map[string][]byte{
		"report.yaml": fmt.Appendf(nil, "apiVersion: %s\nkind: Report\nversion: 1.0.0\nrenderer: %s\nentry: report.tsx\ndata: data.json\n", Protocol, item.ID),
		"data.json":   append(raw, '\n'),
	}
	entries, err := fs.ReadDir(starterFiles, "starter")
	if err != nil {
		return nil, err
	}
	for _, entry := range entries {
		data, err := starterFiles.ReadFile("starter/" + entry.Name())
		if err != nil {
			return nil, err
		}
		files[entry.Name()] = data
	}
	return files, nil
}
