package output

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestSuccessEnvelopeRequiresObjectData(t *testing.T) {
	encoded, err := Success("mf", map[string]any{"items": []string{}})
	if err != nil || string(encoded) != "{\"command\":\"mf\",\"data\":{\"items\":[]},\"status\":\"ok\"}\n" {
		t.Fatalf("success wire: %s (%v)", encoded, err)
	}
	if _, err := Success("mf", []string{"already", "a", "list"}); err == nil {
		t.Fatal("data must remain an object")
	}
}

func TestEnvelopeDoesNotDoubleWrapOrRewriteUnicodeAndEscapes(t *testing.T) {
	input := map[string]any{"status": "ok", "command": "mf", "data": map[string]any{"value": "<中>\u2028\u2029", "literal": `\u2028`}}
	raw, err := Success("unused", input)
	if err != nil {
		t.Fatal(err)
	}
	want := "{\"command\":\"mf\",\"data\":{\"literal\":\"\\\\u2028\",\"value\":\"<中>\u2028\u2029\"},\"status\":\"ok\"}\n"
	if string(raw) != want {
		t.Fatalf("wire changed: %q, want %q", raw, want)
	}
}

func TestTypedErrorWireAndExitMapping(t *testing.T) {
	for _, item := range []struct {
		kind string
		code int
	}{
		{"usage", 2}, {"selection_ambiguous", 2}, {"incompatible_schema", 2},
		{"content_conflict", 2},
		{"not_in_mind_repo", 1}, {"parse_error", 1}, {"not_found", 1},
		{"not_implemented", 64}, {"internal", 1},
	} {
		err := &Error{Kind: item.kind, Message: "details"}
		if err.ExitCode() != item.code {
			t.Errorf("%s exit=%d, want %d", item.kind, err.ExitCode(), item.code)
		}
		var envelope map[string]any
		if decodeErr := json.Unmarshal(err.JSON(), &envelope); decodeErr != nil {
			t.Fatal(decodeErr)
		}
		body := envelope["error"].(map[string]any)
		if envelope["command"] != "mf" || envelope["status"] != "error" || body["hint"] != nil {
			t.Fatalf("error envelope: %#v", envelope)
		}
		if _, present := body["hint"]; !present {
			t.Fatal("absent hint must remain JSON null, not be omitted")
		}
	}
	conflict := &Error{Kind: "path_conflict", Message: "conflict", Lifecycle: true}
	if conflict.ExitCode() != 2 {
		t.Fatal("lifecycle errors must use usage exit 2")
	}
}

func TestTableFormattingKeepsIdentityAndHasNoTrailingPadding(t *testing.T) {
	view := List{Headers: []string{"NAME", "COUNT", "DETAIL"}, Rows: [][]ListCell{TextCells("alpha", "12", "long text"), TextCells("b", "3", "short")}, Plural: "items"}
	got := RenderList(view, ListOptions{NoHeaders: true, NoTrunc: true})
	want := "alpha  12     long text\nb      3      short\n"
	if got != want {
		t.Fatalf("table: %q, want %q", got, want)
	}
	for _, line := range strings.Split(strings.TrimSuffix(got, "\n"), "\n") {
		if strings.TrimRight(line, " ") != line {
			t.Fatalf("trailing padding: %q", line)
		}
	}
	if got := RenderList(List{Plural: "items"}, ListOptions{}); got != "No items found.\n" {
		t.Fatalf("empty list: %q", got)
	}
}
