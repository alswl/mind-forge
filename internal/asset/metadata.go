package asset

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/textcase"
)

func Metadata(name string) (uint64, string, error) {
	info, err := os.Stat(name)
	if err != nil {
		return 0, "", err
	}
	file, err := os.Open(name)
	if err != nil {
		return 0, "", err
	}
	defer file.Close()
	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return 0, "", err
	}
	return uint64(info.Size()), hex.EncodeToString(hash.Sum(nil)), nil
}

func assetFileName(name string) string {
	parts := strings.Split(name, string(filepath.Separator))
	for i := len(parts) - 1; i >= 0; i-- {
		switch parts[i] {
		case "", ".":
			continue
		case "..":
			return ""
		default:
			return parts[i]
		}
	}
	return ""
}

func InferKind(name string) string {
	base := assetFileName(name)
	dot := strings.LastIndexByte(base, '.')
	if dot <= 0 {
		return "other"
	}
	switch textcase.ASCIILower(base[dot+1:]) {
	case "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp":
		return "image"
	case "mp4", "mov", "webm", "mkv", "avi":
		return "video"
	case "mp3", "wav", "flac", "ogg", "m4a":
		return "audio"
	default:
		return "other"
	}
}

func RenameName(name string) string {
	base := assetFileName(name)
	if base == "" || !utf8.ValidString(base) {
		return name
	}
	if dot := strings.LastIndexByte(base, '.'); dot > 0 {
		return base[:dot]
	}
	return base
}

func References(project string, index model.Index, name string) (bool, model.JSONArray[model.Reference]) {
	refs := model.JSONArray[model.Reference]{}
	mentioned := false
	if index.Articles == nil {
		return false, refs
	}
	for _, article := range *index.Articles {
		path := article.Path
		refs = append(refs, model.Reference{FromKind: "article", FromID: article.Title, FromPath: &path, Kind: "index"})
		raw, err := os.ReadFile(joinPath(project, article.Path))
		if err == nil && utf8.Valid(raw) && strings.Contains(string(raw), name) {
			mentioned = true
			refs = append(refs, model.Reference{FromKind: "article", FromID: article.Title, FromPath: &path, Kind: "mention"})
		}
	}
	return mentioned, refs
}

func joinPath(root, path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(root, path)
}
