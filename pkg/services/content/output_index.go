package content

import (
	"path/filepath"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

// IndexOutputProjection updates only the derived output section. Authoritative
// registrations and unrelated projections retain their existing values.
func IndexOutputProjection(root, project string) error {
	minds, err := Discover(root, project)
	if err != nil {
		return err
	}
	outputs := []model.Output{}
	for _, mind := range minds {
		outputs = append(outputs, mind.Outputs...)
	}
	values, err := records(outputs)
	if err != nil {
		return err
	}
	path := filepath.Join(project, "mind-index.yaml")
	index, err := store.ReadIndex(path, true)
	if err != nil {
		return err
	}
	updated := index.Value
	extra := []model.ExtraField{}
	for _, field := range updated.Extra {
		if field.Key != "outputs" {
			extra = append(extra, field)
		}
	}
	updated.Extra = append(extra, model.ExtraField{Key: "outputs", Value: values})
	return index.Write(path, updated, false)
}
