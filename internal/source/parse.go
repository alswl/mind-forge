package source

import (
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/textcase"
)

func fileName(value string) string {
	parts := strings.Split(value, string(filepath.Separator))
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

func fileParts(value string) (string, string) {
	name := fileName(value)
	if dot := strings.LastIndexByte(name, '.'); dot > 0 {
		return name[:dot], name[dot+1:]
	}
	return name, ""
}

func DeriveNameFromPath(value string) (string, error) {
	stem, _ := fileParts(value)
	if stem == "" || !utf8.ValidString(stem) {
		return "", output.Usage("cannot derive name from path '"+value+"'", "pass --name <STRING>")
	}
	return stem, nil
}

func InferFileKind(value string) string {
	_, extension := fileParts(value)
	if textcase.ASCIIEqualFold(extension, "pdf") {
		return "pdf"
	}
	return "file"
}

func FileExtensionForURL(value, kind string) string {
	if at := strings.IndexAny(value, "?#"); at >= 0 {
		value = value[:at]
	}
	_, extension := fileParts(value)
	if extension != "" && len(extension) <= 10 && utf8.ValidString(extension) {
		return extension
	}
	if kind == "rss" {
		return "xml"
	}
	return "html"
}

func SuggestUniqueName(value, sourcesDirectory string) string {
	stem, _ := fileParts(value)
	relative := filepath.Clean(value)
	base := filepath.Clean(sourcesDirectory)
	if relative == base {
		relative = ""
	} else if strings.HasPrefix(relative, base+string(filepath.Separator)) {
		relative = strings.TrimPrefix(relative, base+string(filepath.Separator))
	}
	parts := strings.Split(filepath.ToSlash(relative), "/")
	if filepath.IsAbs(relative) {
		parts = append([]string{"/"}, parts[1:]...)
	}
	if len(parts) > 1 && parts[0] != "" && parts[0] != stem {
		return parts[0] + "-" + stem
	}
	parent := fileName(filepath.Dir(filepath.Clean(value)))
	if parent != "" {
		return parent + "-" + stem
	}
	return stem
}

func ResolveSelector(items []model.Source, selector string) (string, error) {
	matches := []model.Source{}
	for _, item := range items {
		if item.Name == selector || item.Path != nil && *item.Path == selector {
			matches = append(matches, item)
		}
	}
	switch len(matches) {
	case 0:
		hint := "use `mf source list` to see available sources"
		return "", &output.Error{Kind: "not_found", Message: "source '" + selector + "' not found", Hint: &hint}
	case 1:
		return matches[0].Name, nil
	default:
		candidates := make([]string, 0, len(matches))
		for _, item := range matches {
			path := "no path"
			if item.Path != nil {
				path = *item.Path
			}
			candidates = append(candidates, item.Name+" ("+path+")")
		}
		return "", output.Usage("source selector '"+selector+"' matches multiple registrations: "+strings.Join(candidates, ", "), "use an unambiguous source name or path")
	}
}
