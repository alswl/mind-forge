package lifecycle

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"
)

func TestProtocolJSONRejectsInvalidUnicodeWithoutChangingLiteralEscapes(t *testing.T) {
	for _, raw := range [][]byte{
		[]byte(`{"kind":"directory","mode":493,"note":"\ud800"}`),
		[]byte(`{"kind":"directory","mode":493,"note":"\udc00"}`),
		[]byte(`{"kind":"directory","mode":493,"note":"\ud800\u0041"}`),
		append(append([]byte(`{"kind":"directory","mode":493,"note":"`), 255), []byte(`"}`)...),
	} {
		var entry Entry
		if err := json.Unmarshal(raw, &entry); err == nil {
			t.Fatalf("invalid Unicode accepted: %q", raw)
		}
	}
	for _, raw := range []string{
		`{"kind":"directory","mode":493,"note":"\ud83d\ude80"}`,
		`{"kind":"directory","mode":493,"note":"\\ud800"}`,
		`{"kind":"directory","mode":493,"note":"�"}`,
	} {
		var entry Entry
		if err := json.Unmarshal([]byte(raw), &entry); err != nil {
			t.Fatalf("valid Unicode rejected: %s (%v)", raw, err)
		}
	}
}

func TestSavedPlanAndJournalMatchFrozenRustWire(t *testing.T) {
	raw, err := os.ReadFile("testdata/legacy-encoding.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture map[string]string
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	var plan Plan
	if err := json.Unmarshal([]byte(fixture["plan"]), &plan); err != nil {
		t.Fatal(err)
	}
	encoded, err := CanonicalJSON(plan)
	if err != nil || string(encoded) != fixture["plan"] {
		t.Fatalf("plan wire changed: %s (%v)", encoded, err)
	}
	if digest, err := plan.ComputeDigest(); err != nil || digest != fixture["plan_digest"] {
		t.Fatalf("legacy plan digest changed: %s (%v)", digest, err)
	}
	var journal Journal
	if err := json.Unmarshal([]byte(fixture["journal"]), &journal); err != nil {
		t.Fatal(err)
	}
	encoded, err = CanonicalJSON(journal)
	if err != nil || string(encoded) != fixture["journal"] {
		t.Fatalf("journal wire changed: %s (%v)", encoded, err)
	}
}

func TestEntryWireUsesNumericBytesAndDeclaredFieldOrder(t *testing.T) {
	entry := Entry{Kind: "file", Bytes: []byte{0, 10, 128, 255}, Mode: 0o644}
	encoded, err := CanonicalJSON(entry)
	want := `{"kind":"file","bytes":[0,10,128,255],"mode":420}`
	if err != nil || string(encoded) != want {
		t.Fatalf("entry wire: %s, want %s (%v)", encoded, want, err)
	}
	var decoded Entry
	if err := json.Unmarshal(encoded, &decoded); err != nil || !bytes.Equal(decoded.Bytes, entry.Bytes) {
		t.Fatalf("byte round trip: %#v (%v)", decoded, err)
	}
	if _, err := CanonicalJSON(Entry{Kind: "directory", Mode: 0o755}); err != nil {
		t.Fatal(err)
	}
}

func TestCanonicalSnapshotOrdersKeysAndPreservesUTF8(t *testing.T) {
	snapshot := Snapshot{
		"中\u2028<>&": {Kind: "directory", Mode: 0o700},
		"a\"\n":      {Kind: "file", Bytes: []byte{}, Mode: 0o600},
	}
	encoded, err := CanonicalJSON(snapshot)
	want := "{\"a\\\"\\n\":{\"kind\":\"file\",\"bytes\":[],\"mode\":384},\"中\u2028<>&\":{\"kind\":\"directory\",\"mode\":448}}"
	if err != nil || string(encoded) != want {
		t.Fatalf("snapshot canonical bytes: %s, want %s (%v)", encoded, want, err)
	}
}

func TestPlanDigestIsTheLegacyTupleNotThePlanObject(t *testing.T) {
	plan := Plan{Version: 1, Root: "/fixture", Operation: "rename", Watches: []string{"."}, Before: Snapshot{}, After: Snapshot{}}
	wantBytes := `[1,"/fixture","rename",["."],{},{}]`
	hash := sha256.Sum256([]byte(wantBytes))
	wantDigest := hex.EncodeToString(hash[:])
	digest, err := plan.ComputeDigest()
	if err != nil || digest != wantDigest {
		t.Fatalf("digest: %s, want %s (%v)", digest, wantDigest, err)
	}
	plan.Digest = "excluded-from-the-digest"
	if again, err := plan.ComputeDigest(); err != nil || again != wantDigest {
		t.Fatalf("digest must exclude its own field: %s (%v)", again, err)
	}
}

func TestEntryRejectsInvalidWire(t *testing.T) {
	for _, input := range []string{
		`{"kind":"file","bytes":"AA==","mode":420}`,
		`{"kind":"file","bytes":[256],"mode":420}`,
		`{"kind":"file","bytes":[-1],"mode":420}`,
		`{"kind":"file","bytes":null,"mode":420}`,
		`{"kind":"file","bytes":[null],"mode":420}`,
		`{"kind":"file","bytes":[],"mode":-1}`,
		`{"kind":"file","bytes":[]}`,
		`{"kind":"unknown","mode":420}`,
		`{"kind":"file","mode":493,"bytes":[],"bytes":[]}`,
		`{"kind":"directory","mode":493,"mode":493}`,
	} {
		var entry Entry
		if err := json.Unmarshal([]byte(input), &entry); err == nil {
			t.Errorf("accepted invalid old wire: %s", input)
		}
	}
}

func TestDirectoryWireIgnoresUnknownBytesFields(t *testing.T) {
	var entry Entry
	if err := json.Unmarshal([]byte(`{"kind":"directory","mode":493,"bytes":null,"bytes":"ignored"}`), &entry); err != nil {
		t.Fatal(err)
	}
	raw, err := CanonicalJSON(entry)
	if err != nil || string(raw) != `{"kind":"directory","mode":493}` {
		t.Fatalf("directory wire: %s (%v)", raw, err)
	}
}

func TestPlanAndJournalRejectMissingOrNullProtocolFields(t *testing.T) {
	for _, raw := range []string{
		`{}`,
		`{"version":1,"root":"/fixture","operation":"x","watches":null,"before":{},"after":{},"digest":"x"}`,
		`{"version":1,"root":"/fixture","operation":"x","watches":[null],"before":{},"after":{},"digest":"x"}`,
		`{"version":1,"root":"/fixture","operation":"x","watches":[],"before":null,"after":{},"digest":"x"}`,
	} {
		var plan Plan
		if err := json.Unmarshal([]byte(raw), &plan); err == nil {
			t.Errorf("invalid protocol plan accepted: %s", raw)
		}
	}
	for _, raw := range []string{`{}`, `{"plan":null,"phase":"prepared"}`, `{"plan":{},"phase":"unknown"}`} {
		var journal Journal
		if err := json.Unmarshal([]byte(raw), &journal); err == nil {
			t.Errorf("invalid protocol journal accepted: %s", raw)
		}
	}
}
