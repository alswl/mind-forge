package content

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestInspectProjectPreservesBothLayoutCountsAndLegacyDetails(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		t.Run(layout, func(t *testing.T) {
			root := canonicalPath(t.TempDir())
			putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: "+layout+"\nminds: {empty: {}, idea: {}}\n")
			path := "outputs/idea/draft.md"
			if layout == "bundle" {
				path = "idea/outputs/draft.md"
			}
			putContent(t, root, path, "# Synthetic\n")
			result, err := InspectProject(root, root, ".")
			require.NoError(t, err)
			require.Nil(t, result.Legacy)
			require.Equal(t, layout, result.Mind.Layout)
			require.Equal(t, 2, len(result.Mind.Minds))
			require.Equal(t, 1, result.Mind.OutputCount)
			require.Equal(t, "", result.Mind.Identity)
		})
	}
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles: [{title: Synthetic, article_path: docs/a.md}]\nsources: [{name: source, type: file, path: sources/a.md}]\nassets: [{name: asset, type: image, path: assets/a.png}]\n")
	result, err := InspectProject(root, root, ".")
	require.NoError(t, err)
	require.Nil(t, result.Mind)
	require.Equal(t, ".", result.Legacy.Name)
	require.EqualValues(t, 1, result.Legacy.ArticleCount)
	require.EqualValues(t, 1, result.Legacy.SourceCount)
	require.EqualValues(t, 1, result.Legacy.AssetCount)
}
func TestInspectProjectReturnsErrorsBeforeAnyOutcome(t *testing.T) {
	root := canonicalPath(t.TempDir())
	_, err := InspectProject(root, root, "missing")
	require.Error(t, err)
	putContent(t, root, "mind.yaml", "schema: '1'\ncontent_layout: category\n")
	putContent(t, root, "outputs/idea/same.md", "# Synthetic\n")
	putContent(t, root, "outputs/idea/same/body.md", "# Conflict\n")
	result, err := InspectProject(root, root, ".")
	require.Error(t, err)
	require.Nil(t, result.Mind)
	require.Nil(t, result.Legacy)
}

func TestInspectProjectPreservesConfigErrors(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: [broken\n")
	result, err := InspectProject(root, root, ".")
	require.Error(t, err)
	require.Nil(t, result.Mind)
	require.Nil(t, result.Legacy)
}
