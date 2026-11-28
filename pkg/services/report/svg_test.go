package report

import (
	"github.com/stretchr/testify/require"
	"testing"
)

func TestFragmentSVG(t *testing.T) {
	f := Fragment{Width: 100, Height: 50, Components: []Placement{{Entry: "first"}, {Entry: "second"}}}
	panel := `<svg><use href="#mark"/><rect id="mark"/></svg>`
	wrap := func(inner string) []byte {
		return []byte(`<svg xmlns="http://www.w3.org/2000/svg">` + inner + "</svg>")
	}
	raw, err := normalizeSVG(wrap(panel+panel), f)
	require.NoError(t, err)
	for _, id := range []string{"c1-mark", "c2-mark"} {
		require.Contains(t, string(raw), `href="#`+id+`"`)
		require.Contains(t, string(raw), `id="`+id+`"`)
	}
	for _, bad := range []string{`<foreignObject/>`, `<script/>`, `<use href="external.svg"/>`, `<rect fill="url(https://example.invalid/a)"/>`, `<rect onload="bad"/>`, `<rect id="a"/><rect id="a"/>`, `<use href="#missing"/>`} {
		_, err := normalizeSVG(wrap(panel+"<svg>"+bad+"</svg>"), f)
		require.ErrorContains(t, err, "second")
	}
}
