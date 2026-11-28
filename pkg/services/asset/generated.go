package asset

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/alswl/mind-forge/pkg/common/model"
	dal "github.com/alswl/mind-forge/pkg/dal/report"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

var ErrGeneratedConflict = errors.New("generated asset conflict")

// PublishGenerated runs under the caller's repository write lock.
func (service Service) PublishGenerated(root *os.Root, path, owner string, raw []byte) error {
	for _, p := range []string{path, "mind-index.yaml"} {
		if err := dal.Check(root, p, true); err != nil {
			return err
		}
	}
	document, err := readIndex(root.Name())
	if err != nil {
		return err
	}
	items := assetEntries(document.Value)
	item := model.Asset{Name: path, Path: path, Kind: InferKind(path), Tags: model.JSONArray[string]{}, AddedAt: service.timestamp(), GeneratedBy: owner}
	matched := -1
	for i, prior := range items {
		if prior.Path != path && prior.Name == item.Name {
			return fmt.Errorf("%w: asset name already registered at %s", ErrGeneratedConflict, prior.Path)
		}
		if prior.Path != path {
			continue
		}
		if matched >= 0 || prior.GeneratedBy != owner {
			return fmt.Errorf("%w: asset ownership conflict at %s", ErrGeneratedConflict, path)
		}
		matched = i
		item = prior
	}
	dest := filepath.Join(root.Name(), path)
	if _, err = root.Lstat(path); err == nil {
		if matched < 0 {
			return fmt.Errorf("%w: unregistered asset at %s", ErrGeneratedConflict, path)
		}
		if err = dal.Regular(root, path); err != nil {
			return err
		}
		_, hash, err := Metadata(dest)
		if err != nil {
			return err
		}
		if hash != item.Hash {
			return fmt.Errorf("%w: asset modified at %s", ErrGeneratedConflict, path)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	item.Hash = fmt.Sprintf("%x", sha256.Sum256(raw))
	item.Size = uint64(len(raw))
	if matched < 0 {
		items = append(items, item)
	} else {
		items[matched] = item
	}
	sort.SliceStable(items, func(i, j int) bool { return items[i].Name < items[j].Name })
	updated := document.Value
	updated.Assets = &items
	if err = root.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	return store.PublishFile(dest, raw, 0644, "", func() (bool, error) { return service.save(root.Name(), document, updated) })
}
