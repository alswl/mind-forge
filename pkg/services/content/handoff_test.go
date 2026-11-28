package content

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

const testHandoff = "> [!mf-private] Handoff\n> Decision: Saturday\n> Rework: Roster changed\n> Open items: Fees\n> Next step: Confirm hours\n"

func TestReadOutputHandoffUsesSelectedAuthorFiles(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		for _, shape := range []string{"file", "directory"} {
			t.Run(layout+"/"+shape, func(t *testing.T) {
				project := t.TempDir()
				putContent(t, project, "mind.yaml", "schema: '1'\ncontent_layout: "+layout+"\n")
				prefix := "outputs/idea"
				if layout == "bundle" {
					prefix = "idea/outputs"
				}
				putContent(t, project, prefix+"/sibling.md", testHandoff)
				path := prefix + "/draft.md"
				if shape == "directory" {
					putContent(t, project, prefix+"/draft/01-title.md", "# Handoff\n")
					putContent(t, project, prefix+"/draft/archive/nested.md", testHandoff)
					path = prefix + "/draft/02-content.md"
				}
				public := "# Draft\n\n```markdown\n" + testHandoff + "```\n"
				putContent(t, project, path, public)
				item, err := ResolveOutput(project, project, "::idea::draft")
				require.NoError(t, err)
				report, err := ReadOutputHandoff(t.Context(), project, item)
				require.NoError(t, err)
				require.Equal(t, item, report.Output)
				require.Equal(t, filepath.Join(project, item.Path), report.EditablePath)
				require.Equal(t, HandoffMissing, report.Handoff.Status)
				require.Nil(t, report.Handoff.Record)
				require.Empty(t, report.Handoff.Locations)

				body := public + "\n" + testHandoff
				putContent(t, project, path, body)
				report, err = ReadOutputHandoff(t.Context(), project, item)
				require.NoError(t, err)
				require.Equal(t, HandoffFound, report.Handoff.Status)
				require.Equal(t, "Saturday", report.Handoff.Record.Decision)
				fullPath := filepath.Join(project, path)
				require.Equal(t, []HandoffLocation{{fullPath, 11, 15}}, report.Handoff.Locations)
				data, err := os.ReadFile(fullPath)
				require.NoError(t, err)
				require.Equal(t, body, string(data))

				if shape == "directory" {
					putContent(t, project, prefix+"/draft/03-extra.md", testHandoff)
				} else {
					putContent(t, project, path, body+"\n"+testHandoff)
				}
				report, err = ReadOutputHandoff(t.Context(), project, item)
				require.NoError(t, err)
				require.Equal(t, HandoffAmbiguous, report.Handoff.Status)
				require.Nil(t, report.Handoff.Record)
				require.Len(t, report.Handoff.Locations, 2)
				require.Equal(t, fullPath, report.Handoff.Locations[0].Path)
				require.NotEmpty(t, report.Handoff.Diagnostics)
			})
		}
	}
}

func TestReadOutputHandoffRejectsSymlinkedSiblingBlock(t *testing.T) {
	project := t.TempDir()
	putContent(t, project, "mind.yaml", "schema: '1'\ncontent_layout: category\n")
	putContent(t, project, "outputs/idea/draft/01-title.md", "# Draft\n")
	putContent(t, project, "outputs/idea/sibling.md", testHandoff)
	item, err := ResolveOutput(project, project, "::idea::draft")
	require.NoError(t, err)
	link := filepath.Join(project, "outputs/idea/draft/02-link.md")
	require.NoError(t, os.Symlink("../sibling.md", link))
	_, err = ReadOutputHandoff(t.Context(), project, item)
	require.ErrorContains(t, err, "not a regular Markdown file: "+link)
}
