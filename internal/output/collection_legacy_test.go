package output

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestLegacyJSONCollectionWrapsItems(t *testing.T) {
	items := []map[string]any{{"identity": "demo", "docs": 4}}
	result := JSONCollection("projects", items)
	rows, ok := result["projects"].([]map[string]any)
	if !ok || len(rows) != 1 {
		t.Fatal(result)
	}
	if got := rows[0]; got["identity"] != "demo" || got["docs"] != 4 {
		t.Fatal(got)
	}
}
func TestJSONCollectionKeepsEmptyArrays(t *testing.T) {
	var items []map[string]any
	raw, err := json.Marshal(JSONCollection("projects", items))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), `"projects":[]`) {
		t.Fatal(string(raw))
	}
}
