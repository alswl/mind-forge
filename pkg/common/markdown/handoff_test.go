package markdown

import (
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestHandoffCalloutsReadOnlyPrivateAuthorContent(t *testing.T) {
	lines := []string{
		"---", "example: |", "  ```", "---",
		"# Handoff", "Decision: public", "",
		"~~~markdown", "> [!mf-private] Handoff", "> Decision: fenced", "~~~", "",
		"> [!mf-private] Notes", "> ~~~markdown", "> [!mf-private] Handoff", "> Decision: quoted fence", "> ~~~", "",
		"> [!mf-private] Handoff",
		"> Decision: Only Saturday",
		"> Rework: [Roster](../sources/roster.md)",
		">   Weekdays have no coverage.",
		">",
		"> Open items:",
		"> Next step: Confirm hours",
		"", "# Public",
	}
	records := HandoffCallouts(strings.Join(lines, "\r\n"))
	require.Len(t, records, 1)
	record := records[0]
	require.Empty(t, record.Diagnostics)
	require.Equal(t, 19, record.StartLine)
	require.Equal(t, 25, record.EndLine)
	require.Equal(t, HandoffFields{
		Decision: "Only Saturday", Rework: "[Roster](../sources/roster.md)\nWeekdays have no coverage.",
		NextStep: "Confirm hours",
	}, record.Fields)
}

func TestHandoffCalloutsDiagnoseInvalidFields(t *testing.T) {
	records := HandoffCallouts("> [!mf-private] Handoff\n>   early continuation\n> Decision:\n> Decision: duplicate\n> Rework: reason\n> Unknown: value\n> Open items:\n")
	require.Len(t, records, 1)
	require.Equal(t, []string{
		"line 2: expected a handoff field or a quoted continuation",
		"line 4: duplicate Decision field",
		"line 6: expected a handoff field or a quoted continuation",
		"missing Next step field",
	}, records[0].Diagnostics)
	require.Equal(t, 7, records[0].EndLine)
}
