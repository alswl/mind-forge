package content

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type BlockEditReport struct {
	ArticlePath string                  `json:"article_path"`
	OldPath     *string                 `json:"old_path"`
	NewPath     *string                 `json:"new_path"`
	Order       model.JSONArray[string] `json:"order"`
	DryRun      bool                    `json:"dry_run"`
}
type BlockRemoveReport struct {
	ArticlePath     string `json:"article_path"`
	RemovedFilename string `json:"removed_filename"`
	RemainingBlocks int    `json:"remaining_blocks"`
	DryRun          bool   `json:"dry_run"`
}
type BlockRenameReport struct {
	OldFilename string `json:"old_filename"`
	NewFilename string `json:"new_filename"`
	ArticlePath string `json:"article_path"`
	Force       bool   `json:"force"`
}
type BlockEditOptions struct {
	After  *string
	Start  uint64
	DryRun bool
}

func ListSectionFiles(project, article string) ([]string, error) {
	path, err := CheckedPath(project, article)
	if err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(path)
	if err != nil {
		return nil, err
	}
	files := []string{}
	for _, entry := range entries {
		name := entry.Name()
		if utf8.ValidString(name) && filepath.Ext(name) == ".md" {
			files = append(files, article+"/"+name)
		}
	}
	sort.Strings(files)
	return files, nil
}

func blockNotFound(article, block string) error {
	hint := "use `mf article show <article>` to list blocks"
	return &output.Error{Kind: "not_found", Message: fmt.Sprintf("block '%s' not found in article '%s'", block, article), Hint: &hint}
}

func ResolveBlockFilename(files []string, article, block string) (string, error) {
	if strings.ContainsAny(block, "/\\") {
		normalized := strings.ReplaceAll(block, "\\", "/")
		rest, ok := strings.CutPrefix(normalized, strings.TrimRight(article, "/")+"/")
		if ok && rest != "" && !strings.Contains(rest, "/") {
			return ResolveBlockFilename(files, article, rest)
		}
		return "", blockNotFound(article, block)
	}
	if strings.Contains(block, ".") {
		if !strings.HasSuffix(block, ".md") {
			return "", output.Usage(fmt.Sprintf("block identifier '%s' must be a .md file", block), "use the filename (e.g. '02-notes.md') or the slug (e.g. 'notes')")
		}
		for _, file := range files {
			if filepath.Base(file) == block {
				return block, nil
			}
		}
		return "", blockNotFound(article, block)
	}
	sanitized := ArticleFilename(block)
	candidates := []string{}
	for _, file := range files {
		name := filepath.Base(file)
		if strings.TrimSuffix(name, ".md") == sanitized {
			candidates = append(candidates, name)
		}
	}
	if len(candidates) == 0 {
		for _, file := range files {
			name := filepath.Base(file)
			slug := blockFilenameSlug(name)
			if slug != "" && slug == sanitized {
				candidates = append(candidates, name)
			}
		}
	}
	switch len(candidates) {
	case 0:
		return "", blockNotFound(article, block)
	case 1:
		return candidates[0], nil
	default:
		return "", output.Usage(fmt.Sprintf("multiple blocks match '%s' in article '%s'", block, article), "use the full filename (e.g. '02-notes.md') to disambiguate")
	}
}

func blockNumberPrefix(filename string) string {
	stem := strings.TrimSuffix(filename, ".md")
	return stem[:len(stem)-len(strings.TrimLeft(stem, "0123456789"))]
}

func blockFilenameSlug(filename string) string {
	stem := strings.TrimSuffix(filename, ".md")
	return strings.TrimLeft(strings.TrimPrefix(stem, blockNumberPrefix(filename)), "-")
}

func blockSlug(filename string) string {
	stem := filename
	for strings.HasSuffix(stem, ".md") {
		stem = strings.TrimSuffix(stem, ".md")
	}
	if _, slug, ok := strings.Cut(stem, "-"); ok {
		return slug
	}
	return filename
}

type blockBody struct{ name, body string }

func readBlocks(project, article string, files []string) ([]blockBody, error) {
	blocks := make([]blockBody, 0, len(files))
	for _, file := range files {
		name := filepath.Base(file)
		path, err := CheckedPath(project, article+"/"+name)
		if err != nil {
			return nil, err
		}
		body, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if !utf8.Valid(body) {
			return nil, fmt.Errorf("stream did not contain valid UTF-8: %s", path)
		}
		blocks = append(blocks, blockBody{name, string(body)})
	}
	return blocks, nil
}

func rewriteBlocks(project, article string, blocks []blockBody, start uint64, dryRun bool) (model.JSONArray[string], error) {
	order := model.JSONArray[string]{}
	files := map[string][]byte{}
	for i, block := range blocks {
		if uint64(i) > ^uint64(0)-start {
			return nil, output.Usage("block numbering exceeds supported range", "")
		}
		name := fmt.Sprintf("%02d-%s.md", start+uint64(i), block.name)
		order = append(order, name)
		files[name] = []byte(block.body)
	}
	if !dryRun {
		path, err := CheckedPath(project, article)
		if err != nil {
			return nil, err
		}
		if err := store.ReplaceDirectory(path, files); err != nil {
			return nil, err
		}
	}
	return order, nil
}

func blockInsertion(files []string, article string, after *string) (int, error) {
	if after == nil {
		return len(files), nil
	}
	resolved, err := ResolveBlockFilename(files, article, *after)
	if err != nil {
		return 0, err
	}
	for i, file := range files {
		if filepath.Base(file) == resolved {
			return i + 1, nil
		}
	}
	return 0, blockNotFound(article, *after)
}

func NewBlock(project, article, slug string, options BlockEditOptions) (BlockEditReport, error) {
	report := BlockEditReport{ArticlePath: article, DryRun: options.DryRun}
	files, err := ListSectionFiles(project, article)
	if err != nil {
		return report, err
	}
	if len(files) == 0 {
		return report, &output.Error{Kind: "not_found", Message: "no blocks found"}
	}
	normalized := ArticleFilename(slug)
	if normalized == "untitled" {
		return report, output.Usage("block slug must contain an alphanumeric character", "")
	}
	for _, file := range files {
		if blockSlug(filepath.Base(file)) == normalized {
			return report, output.Usage(fmt.Sprintf("block slug '%s' already exists", slug), "choose a unique block slug")
		}
	}
	at, err := blockInsertion(files, article, options.After)
	if err != nil {
		return report, err
	}
	blocks, err := readBlocks(project, article, files)
	if err != nil {
		return report, err
	}
	for i := range blocks {
		blocks[i].name = blockSlug(blocks[i].name)
	}
	body := "# " + strings.TrimSpace(slug) + "\n\n"
	if config, err := app.LoadProjectConfig(project, project); err == nil && config != nil {
		enabled := typoraEnabled(config)
		if enabled {
			layout, err := app.ResolveAndValidateLayout(project, config)
			if err != nil {
				return report, err
			}
			assets := layout.Assets
			if !filepath.IsAbs(assets) {
				target := filepath.Join(project, assets)
				from := filepath.Join(project, article)
				if canonical, err := filepath.EvalSymlinks(target); err == nil {
					target = canonical
				}
				if canonical, err := filepath.EvalSymlinks(from); err == nil {
					from = canonical
				}
				assets, err = filepath.Rel(from, target)
				if err != nil {
					return report, err
				}
				assets = filepath.ToSlash(assets)
			}
			body = InjectTyporaFrontMatter(body, assets)
		}
	}
	blocks = append(blocks, blockBody{})
	copy(blocks[at+1:], blocks[at:])
	blocks[at] = blockBody{normalized, body}
	report.Order, err = rewriteBlocks(project, article, blocks, options.Start, options.DryRun)
	if err != nil {
		return report, err
	}
	path := article + "/" + report.Order[at]
	report.NewPath = &path
	return report, nil
}

func MoveBlock(project, article, block string, options BlockEditOptions) (BlockEditReport, error) {
	report := BlockEditReport{ArticlePath: article, DryRun: options.DryRun}
	files, err := ListSectionFiles(project, article)
	if err != nil {
		return report, err
	}
	selected, err := ResolveBlockFilename(files, article, block)
	if err != nil {
		return report, err
	}
	remaining := make([]string, 0, len(files)-1)
	for _, file := range files {
		if filepath.Base(file) != selected {
			remaining = append(remaining, file)
		}
	}
	at, err := blockInsertion(remaining, article, options.After)
	if err != nil {
		return report, err
	}
	reordered := append(remaining, "")
	copy(reordered[at+1:], reordered[at:])
	reordered[at] = article + "/" + selected
	blocks, err := readBlocks(project, article, reordered)
	if err != nil {
		return report, err
	}
	for i := range blocks {
		blocks[i].name = blockSlug(blocks[i].name)
	}
	report.Order, err = rewriteBlocks(project, article, blocks, options.Start, options.DryRun)
	if err != nil {
		return report, err
	}
	oldPath, newPath := article+"/"+selected, article+"/"+report.Order[at]
	report.OldPath, report.NewPath = &oldPath, &newPath
	return report, nil
}

func RenumberBlocks(project, article string, start uint64, dryRun bool) (BlockEditReport, error) {
	report := BlockEditReport{ArticlePath: article, DryRun: dryRun}
	files, err := ListSectionFiles(project, article)
	if err != nil {
		return report, err
	}
	blocks, err := readBlocks(project, article, files)
	if err != nil {
		return report, err
	}
	for i := range blocks {
		blocks[i].name = blockSlug(blocks[i].name)
	}
	report.Order, err = rewriteBlocks(project, article, blocks, start, dryRun)
	return report, err
}

func directoryBlocks(project, article, action string) ([]string, error) {
	path, err := CheckedPath(project, article)
	if err != nil {
		return nil, err
	}
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		command := action
		if action == "removal" {
			command = "remove"
		}
		return nil, output.Usage(fmt.Sprintf("'%s' is not a directory article", article), fmt.Sprintf("block %s only works on directory articles. Use `mf article %s` to %s a single-file article.", action, command, command))
	}
	files, err := ListSectionFiles(project, article)
	if err != nil {
		return nil, err
	}
	if len(files) == 0 {
		hint := "directory articles must have at least one .md block file"
		return nil, &output.Error{Kind: "not_found", Message: fmt.Sprintf("no block files found in article '%s'", article), Hint: &hint}
	}
	return files, nil
}

func RemoveBlock(project, article, block string, dryRun bool) (BlockRemoveReport, error) {
	report := BlockRemoveReport{ArticlePath: article, DryRun: dryRun}
	files, err := directoryBlocks(project, article, "removal")
	if err != nil {
		return report, err
	}
	if len(files) == 1 {
		return report, output.Usage(fmt.Sprintf("cannot remove the last remaining block in article '%s'", article), "use `mf article remove` to delete the whole article, or `mf article convert --to-single-file` to collapse it into a single file")
	}
	name, err := ResolveBlockFilename(files, article, block)
	if err != nil {
		return report, err
	}
	path, err := CheckedPath(project, article+"/"+name)
	if err != nil {
		return report, err
	}
	if _, err := os.Stat(path); err != nil {
		return report, err
	}
	report.RemovedFilename, report.RemainingBlocks = name, len(files)-1
	if !dryRun {
		err = os.Remove(path)
	}
	return report, err
}

func RenameBlock(project, article, block, newSlug string, force bool) (BlockRenameReport, error) {
	report := BlockRenameReport{ArticlePath: article, Force: force}
	files, err := directoryBlocks(project, article, "rename")
	if err != nil {
		return report, err
	}
	name, err := ResolveBlockFilename(files, article, block)
	if err != nil {
		return report, err
	}
	prefix := blockNumberPrefix(name)
	if prefix == "" {
		return report, fmt.Errorf("cannot extract number prefix from block filename '%s'", name)
	}
	slug := ArticleFilename(newSlug)
	if slug == "untitled" {
		return report, output.Usage(fmt.Sprintf("new slug '%s' produces an empty filename", newSlug), "provide a non-empty slug with at least one alphanumeric character")
	}
	report.OldFilename, report.NewFilename = name, prefix+"-"+slug+".md"
	oldPath, err := CheckedPath(project, article+"/"+name)
	if err != nil {
		return report, err
	}
	newPath, err := CheckedPath(project, article+"/"+report.NewFilename)
	if err != nil {
		return report, err
	}
	if _, err := os.Stat(oldPath); err != nil {
		return report, err
	}
	if name == report.NewFilename {
		return report, nil
	}
	if _, err := os.Stat(newPath); err == nil {
		if !force {
			return report, fileExists(newPath)
		}
		if err := os.Remove(newPath); err != nil {
			return report, err
		}
	} else if !os.IsNotExist(err) {
		return report, err
	}
	return report, os.Rename(oldPath, newPath)
}
