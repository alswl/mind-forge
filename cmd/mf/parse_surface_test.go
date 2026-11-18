package main

import (
	"bytes"
	"compress/gzip"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

// Frozen public parser inputs exercise the parser component directly. No app
// context, filesystem service, binary, or subprocess participates in this test.
func TestParserSurfaceMatchesFrozenObservations(t *testing.T) {
	file, err := os.Open("testdata/parse-surface-legacy.json.gz")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	reader, err := gzip.NewReader(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	var rows []struct {
		ID       string   `json:"id"`
		Argv     []string `json:"argv"`
		Expected struct {
			Code   int    `json:"code"`
			Stdout string `json:"stdout"`
			Stderr string `json:"stderr"`
		} `json:"expected"`
	}
	if err := json.NewDecoder(reader).Decode(&rows); err != nil {
		t.Fatal(err)
	}
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, row := range rows {
		t.Run(row.ID, func(t *testing.T) {
			command, err := NewCommand(nil)
			if err != nil {
				t.Fatal(err)
			}
			command.SetOut(io.Discard)
			command.SetErr(io.Discard)
			command.SetArgs(row.Argv)
			selected, err := command.ExecuteC()
			var failure *parseFailure
			if !errors.As(err, &failure) {
				t.Fatalf("expected parse failure, got %v", err)
			}
			if selected == nil {
				selected = command
			}
			var out bytes.Buffer
			code := renderParseFailure(selected, spec, failure, row.Argv, &out)
			if code != row.Expected.Code || out.String() != row.Expected.Stderr {
				t.Errorf("argv=%q\ncode got %d want %d\nerror got %q\nerror want %q", row.Argv, code, row.Expected.Code, out.String(), row.Expected.Stderr)
			}
		})
	}
}
