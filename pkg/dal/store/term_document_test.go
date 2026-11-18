package store

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/stretchr/testify/require"
)

func TestTermDocumentRetainsOriginalBytesAndProtectedSections(t *testing.T) {
	for _, global := range []bool{false, true} {
		t.Run(map[bool]string{false: "project", true: "global"}[global], func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "terms.yaml")
			raw := []byte("# retained\r\nschema: '1'\r\nunknown: {quoted: 'yes'}\r\npublish_records: [] # protected\r\nterms:\r\n- term: API\r\n")
			require.NoError(t, os.WriteFile(path, raw, 0600))
			doc, err := ReadTermDocument(path, global, nil)
			require.NoError(t, err)
			require.True(t, doc.Existed)
			require.Equal(t, raw, doc.Original)
			terms := []model.Term{{Term: "SDK"}}
			rendered, err := doc.RenderTerms(terms)
			require.NoError(t, err)
			prefix := raw[:bytes.Index(raw, []byte("terms:"))]
			require.True(t, bytes.HasPrefix(rendered, prefix), string(rendered))
			after, err := os.ReadFile(path)
			require.NoError(t, err)
			require.Equal(t, raw, after)
			require.NoError(t, WriteTermDocument(path, rendered))
			after, err = os.ReadFile(path)
			require.NoError(t, err)
			require.Equal(t, rendered, after)
		})
	}
}
func TestTermDocumentMissingSchemaAliasesAndCorruptInputs(t *testing.T) {
	path := filepath.Join(t.TempDir(), "terms.yaml")
	doc, err := ReadTermDocument(path, true, nil)
	require.NoError(t, err)
	require.False(t, doc.Existed)
	require.Empty(t, doc.Terms)
	_, err = os.Stat(path)
	require.ErrorIs(t, err, os.ErrNotExist)
	raw := []byte("schema_version: &schema '1'\nterms: &catalogue\n- term: API\nunknown: *catalogue\n")
	require.NoError(t, os.WriteFile(path, raw, 0600))
	doc, err = ReadTermDocument(path, true, nil)
	require.NoError(t, err)
	rendered, err := doc.RenderTerms([]model.Term{{Term: "Other"}})
	require.NoError(t, err)
	require.NotContains(t, string(rendered), "*catalogue")
	require.Contains(t, string(rendered), "unknown:")
	_, err = ParseYAML(rendered)
	require.NoError(t, err)
	for _, raw := range []string{"terms: []\n", "schema: '2'\nterms: []\n", "schema: '1'\nterms: null\n", "schema: '1'\nterms: [null]\n", "schema: '1'\nschema_version: '1'\n", "schema: [broken\n"} {
		require.NoError(t, os.WriteFile(path, []byte(raw), 0600))
		_, err = ReadTermDocument(path, true, nil)
		require.Error(t, err)
	}
}

func TestTermDocumentKeepsServiceValidationAndNoopBytes(t *testing.T) {
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	raw := []byte("schema: '1'\n# retained\nterms:\n- term: API\n  corrections: [{original: api, correct: API}]\n")
	require.NoError(t, os.WriteFile(path, raw, 0600))
	calls := 0
	doc, err := ReadTermDocument(path, false, func(terms []model.Term) error { calls++; return model.ValidateCorrections(terms) })
	require.NoError(t, err)
	require.Equal(t, 1, calls)
	rendered, err := doc.RenderTerms(doc.Terms)
	require.NoError(t, err)
	require.Equal(t, raw, rendered)
	sentinel := errors.New("synthetic service validation fault")
	_, err = ReadTermDocument(path, false, func([]model.Term) error { return sentinel })
	require.ErrorIs(t, err, sentinel)
	_, err = ReadTermDocument(path, true, func([]model.Term) error { return sentinel })
	require.ErrorIs(t, err, sentinel)
	after, err := os.ReadFile(path)
	require.NoError(t, err)
	require.Equal(t, raw, after)
}
