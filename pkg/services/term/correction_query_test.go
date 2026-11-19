package term

import (
	"os"
	"testing"

	outcome "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/stretchr/testify/require"
)

func TestCorrectionQueryKeepsStrictLoadBeforeCanonicalLookup(t *testing.T) {
	for _, kind := range []string{"project", "global"} {
		t.Run(kind, func(t *testing.T) {
			scope := Scope{Root: t.TempDir(), Kind: kind}
			raw := []byte("schema: '1'\nterms:\n- term: API\n  corrections: [{original: api_, correct: API}]\n")
			require.NoError(t, os.WriteFile(scope.Path(), raw, 0644))
			_, err := scope.QueryCorrections("API")
			require.Error(t, err)
			_, err = scope.QueryCorrections("missing")
			require.ErrorContains(t, err, "identifier-character edges")
			_, err = scope.QueryCorrection("API", "api_")
			require.Error(t, err)
			_, err = scope.Show("API")
			require.NoError(t, err)
			after, err := os.ReadFile(scope.Path())
			require.NoError(t, err)
			require.Equal(t, raw, after)
		})
	}
}
func TestCorrectionQueryRetainsCanonicalAndCorrectionErrorsAndOrder(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "project"}
	raw := []byte("schema: '1'\nterms:\n- term: API\n  aliases: [alias]\n  corrections: [{original: zeta, correct: API}, {original: alpha, correct: API}]\n- term: Empty\n")
	require.NoError(t, os.WriteFile(scope.Path(), raw, 0644))
	tm, err := scope.QueryCorrections("API")
	require.NoError(t, err)
	require.Equal(t, "zeta", tm.Corrections[0].Original)
	correction, err := scope.QueryCorrection("API", "alpha")
	require.NoError(t, err)
	require.Equal(t, "alpha", correction.Original)
	tm, err = scope.QueryCorrections("Empty")
	require.NoError(t, err)
	require.Empty(t, tm.Corrections)
	for _, name := range []string{"alias", "api", "missing"} {
		_, err = scope.QueryCorrections(name)
		var typed *outcome.Error
		require.ErrorAs(t, err, &typed)
		require.Equal(t, "not_found", typed.Kind)
		require.Nil(t, typed.Hint)
		require.Equal(t, "term '"+name+"' not found", typed.Message)
	}
	_, err = scope.QueryCorrection("API", "missing")
	require.EqualError(t, err, `correction "missing" not found on term "API"`)
	_, err = scope.QueryCorrection("API", `odd"input`)
	require.EqualError(t, err, `correction "odd"input" not found on term "API"`)
}
