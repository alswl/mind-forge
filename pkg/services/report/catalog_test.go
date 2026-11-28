package report

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestCatalogReportsInvalidEntriesAndSelectsOnlyDeclaredDependencies(t *testing.T) {
	registry := t.TempDir()
	writeFile(t, registry, "minds.yaml", "schema: '1'\n")
	const base = ".mind-forge/report/"
	const renderer = base + "renderers/weekly/"
	const component = base + "components/delivery/"
	writeFile(t, registry, renderer+"renderer.yaml", `apiVersion: report.mind-forge/v1
kind: Renderer
id: repo/weekly
version: 1.2.3
entry: ./Report.tsx
dataSchema: ./data.schema.json
views: [chart, table]
components: [repo/delivery, builtin/table]
`)
	writeFile(t, registry, renderer+"Report.tsx", "invalid TypeScript demonstrates metadata-only lookup")
	writeFile(t, registry, renderer+"data.schema.json", `{"type":"object","required":["completed"],"properties":{"completed":{"type":"integer"}}}`)
	writeFile(t, registry, renderer+"example.json", `{"completed":7}`)
	writeFile(t, registry, component+"component.yaml", `apiVersion: report.mind-forge/v1
kind: Component
id: repo/delivery
version: 2.0.0
entry: Metric.tsx
propsSchema: props.schema.json
example: example.tsx
`)
	writeFile(t, registry, component+"Metric.tsx", "component")
	writeFile(t, registry, component+"example.tsx", "example")
	writeFile(t, registry, component+"props.schema.json", "{}")
	writeFile(t, registry, base+"renderers/z/renderer.yaml", "apiVersion: report.mind-forge/v2")
	writeFile(t, registry, base+"renderers/a/renderer.yaml", "kind: renderer")
	service := Service{Registry: registry}
	list, err := service.Catalog(Renderer, "")
	require.NoError(t, err)
	require.Len(t, list.Items, 4)
	require.Equal(t, "builtin/brief", list.Items[0].ID)
	require.Equal(t, "repo/weekly", list.Items[3].ID)
	require.Len(t, list.Diagnostics, 2)
	require.Equal(t, "repo/a", list.Diagnostics[0].ID)
	require.Equal(t, "repo/z", list.Diagnostics[1].ID)
	_, err = service.Catalog(Renderer, "repo/Weekly")
	require.ErrorIs(t, err, ErrInvalid)

	scope, request := standalone(t)
	request.Renderer = "repo/weekly"
	writeFile(t, scope.Directory, "data.json", `{"completed":8}`)
	plan, err := service.Plan(t.Context(), scope, request)
	require.NoError(t, err)
	require.Equal(t, filepath.Join(registry, renderer+"Report.tsx"), plan.Definition.Entry)
	require.Len(t, plan.Components, 2)
	require.Equal(t, "repo/delivery", plan.Components[0].ID)
	require.Equal(t, "builtin/table", plan.Components[1].ID)

	require.NoError(t, os.Remove(filepath.Join(registry, component+"Metric.tsx")))
	_, err = service.Plan(t.Context(), scope, request)
	require.ErrorIs(t, err, ErrInvalid)
	require.ErrorContains(t, err, "repo/delivery")
	require.ErrorContains(t, err, "Metric.tsx")

	writeFile(t, registry, renderer+"data.schema.json", `{"$ref":"https://example.invalid/schema"}`)
	_, err = service.Plan(t.Context(), scope, request)
	require.ErrorContains(t, err, "external schema reference")

	writeFile(t, registry, component+"Metric.tsx", "component")
	writeFile(t, registry, renderer+"data.schema.json", "{}")
	writeFile(t, registry, base+"renderers/also-weekly/renderer.yaml", "kind: Renderer\nid: repo/weekly\n")
	_, err = service.Catalog(Renderer, "repo/weekly")
	require.ErrorContains(t, err, "duplicate ID")
}
