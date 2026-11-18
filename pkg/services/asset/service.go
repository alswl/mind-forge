package asset

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

type Service struct {
	Now        func() time.Time
	WriteIndex func(string, []byte) error
}

func (service Service) timestamp() string {
	now := time.Now()
	if service.Now != nil {
		now = service.Now()
	}
	return now.UTC().Format(time.RFC3339)
}

func projectLayout(project string) (model.EffectiveLayout, error) {
	config, err := app.LoadProjectConfig(project, project)
	if err != nil {
		return model.EffectiveLayout{}, err
	}
	if config == nil {
		value := model.DefaultMindConfig()
		config = &value
	}
	layout, _ := app.ResolveEffectiveLayout(config)
	return layout, nil
}

func readIndex(project string) (*store.IndexDocument, error) {
	return store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
}
func assetEntries(index model.Index) []model.Asset {
	if index.Assets == nil {
		return nil
	}
	return append([]model.Asset(nil), (*index.Assets)...)
}

func (service Service) save(project string, document *store.IndexDocument, updated model.Index) (bool, error) {
	raw, err := document.Render(updated)
	if err != nil {
		return false, err
	}
	name := filepath.Join(project, "mind-index.yaml")
	if service.WriteIndex != nil {
		err = service.WriteIndex(name, raw)
	} else {
		err = (store.AtomicWriter{}).Write(name, raw, 0644)
	}
	if err == nil {
		return true, nil
	}
	actual, readError := os.ReadFile(name)
	return readError == nil && bytes.Equal(actual, raw), err
}

func List(project string, filter, kind *string) ([]model.Asset, error) {
	document, err := readIndex(project)
	if err != nil {
		return nil, err
	}
	result := []model.Asset{}
	for _, item := range assetEntries(document.Value) {
		if kind != nil && item.Kind != *kind {
			continue
		}
		if filter != nil {
			match := strings.Contains(textcase.Lower(item.Name), textcase.Lower(*filter))
			for _, tag := range item.Tags {
				match = match || strings.Contains(textcase.Lower(tag), textcase.Lower(*filter))
			}
			if !match {
				continue
			}
		}
		result = append(result, item)
	}
	sort.SliceStable(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, nil
}

func Show(project, selector string) (model.Asset, error) {
	items, err := List(project, nil, nil)
	if err != nil {
		return model.Asset{}, err
	}
	for _, item := range items {
		if item.Path == selector {
			return item, nil
		}
	}
	for _, item := range items {
		if textcase.ASCIIEqualFold(item.Name, selector) {
			return item, nil
		}
	}
	return model.Asset{}, output.Usage("asset '"+selector+"' not found", "use `mf asset list` to see available assets")
}

func refreshEntry(project string, entry *model.Asset) model.AssetUpdateResult {
	result := model.AssetUpdateResult{Path: entry.Path, OldSize: entry.Size, NewSize: entry.Size, OldHash: entry.Hash, NewHash: entry.Hash}
	size, hash, err := Metadata(joinPath(project, entry.Path))
	if err != nil {
		value := "missing_file"
		result.Error = &value
		return result
	}
	result.NewSize, result.NewHash = size, hash
	result.Changed = size != entry.Size || hash != entry.Hash
	entry.Size, entry.Hash = size, hash
	return result
}

func refreshEntries(project string, items []model.Asset) model.JSONArray[model.AssetUpdateResult] {
	results := model.JSONArray[model.AssetUpdateResult]{}
	for i := range items {
		results = append(results, refreshEntry(project, &items[i]))
	}
	sort.SliceStable(results, func(i, j int) bool { return results[i].Path < results[j].Path })
	return results
}

func (service Service) UpdateAll(project string, dryRun bool) (model.JSONArray[model.AssetUpdateResult], error) {
	document, err := readIndex(project)
	if err != nil {
		return nil, err
	}
	items := assetEntries(document.Value)
	results := refreshEntries(project, items)
	changed := false
	for _, result := range results {
		changed = changed || result.Changed
	}
	if changed && !dryRun {
		updated := document.Value
		updated.Assets = &items
		_, err = service.save(project, document, updated)
	}
	return results, err
}

func (service Service) UpdateOne(project, cwd, input string, dryRun bool) (model.AssetUpdateResult, error) {
	layout, err := projectLayout(project)
	if err != nil {
		return model.AssetUpdateResult{}, err
	}
	assets := joinPath(project, layout.Assets)
	resolved := ""
	for _, candidate := range []string{joinPath(project, input), joinPath(assets, input), joinPath(cwd, input)} {
		canonical, err := store.CanonicalWithin(assets, candidate)
		if err == nil {
			resolved = canonical
			break
		}
	}
	if resolved == "" {
		return model.AssetUpdateResult{}, output.Usage(fmt.Sprintf("could not resolve '%s' within %s/", input, layout.Assets), "run `mf asset list`")
	}
	canonical, err := filepath.EvalSymlinks(project)
	if err != nil {
		return model.AssetUpdateResult{}, err
	}
	rel, err := filepath.Rel(canonical, resolved)
	if err != nil {
		return model.AssetUpdateResult{}, err
	}
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return model.AssetUpdateResult{}, output.Usage("resolved path is outside the project root", "run `mf asset list`")
	}
	rel = strings.ReplaceAll(filepath.ToSlash(rel), "\\", "/")
	document, err := readIndex(project)
	if err != nil {
		return model.AssetUpdateResult{}, err
	}
	items := assetEntries(document.Value)
	pos := -1
	for i, item := range items {
		if item.Path == rel {
			pos = i
			break
		}
	}
	if pos < 0 {
		return model.AssetUpdateResult{}, output.Usage("no index entry for '"+rel+"'", "run `mf asset list`")
	}
	if _, err := os.Stat(resolved); os.IsNotExist(err) {
		return model.AssetUpdateResult{}, output.Usage("file not found on disk: "+rel, "run `mf asset index` to reconcile")
	}
	size, hash, err := Metadata(resolved)
	if err != nil {
		return model.AssetUpdateResult{}, err
	}
	result := model.AssetUpdateResult{Path: rel, OldSize: items[pos].Size, OldHash: items[pos].Hash, NewSize: size, NewHash: hash, Changed: size != items[pos].Size || hash != items[pos].Hash}
	if result.Changed && !dryRun {
		items[pos].Size, items[pos].Hash = size, hash
		updated := document.Value
		updated.Assets = &items
		_, err = service.save(project, document, updated)
	}
	return result, err
}

func (service Service) Clean(project string, dryRun bool) (model.AssetCleanReport, error) {
	report := model.AssetCleanReport{StaleEntries: model.JSONArray[string]{}, DryRun: dryRun}
	document, err := readIndex(project)
	if err != nil {
		return report, err
	}
	if document.Value.Assets == nil {
		return report, nil
	}
	kept := []model.Asset{}
	for _, item := range assetEntries(document.Value) {
		if _, err := os.Stat(joinPath(project, item.Path)); err != nil {
			report.StaleEntries = append(report.StaleEntries, item.Path)
		} else {
			kept = append(kept, item)
		}
	}
	report.RemovedCount = uint64(len(report.StaleEntries))
	if !dryRun && report.RemovedCount > 0 {
		updated := document.Value
		updated.Assets = &kept
		_, err = service.save(project, document, updated)
	}
	return report, err
}

func (service Service) SetPublishURL(project, url, channel string, dryRun bool) error {
	if url == "" && channel == "" {
		return output.Usage("at least one of --set-url or --channel is required", "provide --set-url <URL> and/or --channel <NAME>")
	}
	document, err := readIndex(project)
	if err != nil {
		return err
	}
	if dryRun || url == "" || channel == "" {
		return nil
	}
	records := []model.PublishRecord{}
	if document.Value.PublishRecords != nil {
		records = append(records, (*document.Value.PublishRecords)...)
	}
	now := service.timestamp()
	records = append(records, model.PublishRecord{Path: "_publish_settings/" + channel, TargetName: channel, Status: "published", TargetURL: &url, PublishedAt: &now})
	updated := document.Value
	updated.PublishRecords = &records
	_, err = service.save(project, document, updated)
	return err
}
