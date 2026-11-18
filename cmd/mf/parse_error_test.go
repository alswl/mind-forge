package main

import (
	"encoding/json"
	"os"
	"testing"
)

func TestParseErrorTextPreservesFrozenContract(t *testing.T) {
	raw, err := os.ReadFile("testdata/parse-errors-legacy.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Path, Args                            []string
		Kind, Value, Flag, Label, Other, Text string
		Missing, Choices                      []string
	}
	if err := json.Unmarshal(raw, &cases); err != nil {
		t.Fatal(err)
	}
	spec, err := LoadSpec()
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range cases {
		t.Run(test.Kind+"/"+test.Value, func(t *testing.T) {
			item, ok := spec.Lookup(test.Path)
			if !ok {
				t.Fatal(test.Path)
			}
			flag := FlagSpec{Choices: test.Choices}
			if test.Flag != "" {
				flag, ok = spec.Flag(test.Path, "--"+test.Flag)
				if !ok {
					t.Fatal(test.Flag)
				}
			}
			failure := &parseFailure{kind: test.Kind, value: test.Value, flag: flag, label: test.Label, other: test.Other, missing: test.Missing}
			if got := ParseErrorText(spec, item, failure, test.Args); got != test.Text {
				t.Fatalf("parser presentation changed\ngot: %q\nwant: %q", got, test.Text)
			}
		})
	}
}
