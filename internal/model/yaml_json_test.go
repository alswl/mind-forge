package model

import (
	"encoding/json"
	"math"
	"os"
	"reflect"
	"strconv"
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestYAMLJSONModesMatchFrozenRustCorpus(t *testing.T) {
	raw, err := os.ReadFile("testdata/yaml-json-legacy.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []struct {
		Input            string
		Metadata, Target map[string]json.RawMessage
		Index            json.RawMessage
	}
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	compare := func(t *testing.T, raw []byte, want json.RawMessage) {
		t.Helper()
		var actual, expected any
		if err := json.Unmarshal(raw, &actual); err != nil {
			t.Fatal(err)
		}
		if err := json.Unmarshal(want, &expected); err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(actual, expected) {
			t.Fatalf("got %s, want %s", raw, want)
		}
	}
	for _, fixture := range fixtures {
		t.Run(fixture.Input, func(t *testing.T) {
			var source Source
			err := yaml.Unmarshal([]byte("name: example\ndata: "+fixture.Input), &source)
			var encoded []byte
			if err == nil {
				encoded, err = json.Marshal(source)
			}
			if expected, ok := fixture.Metadata["Ok"]; ok {
				if err != nil {
					t.Fatal(err)
				}
				var actual map[string]json.RawMessage
				if err := json.Unmarshal(encoded, &actual); err != nil {
					t.Fatal(err)
				}
				compare(t, []byte(`{"data":`+string(actual["data"])+`}`), expected)
			} else if err == nil {
				t.Fatalf("unsupported flattened metadata accepted: %s", encoded)
			}
			var target PublishTarget
			err = yaml.Unmarshal([]byte("name: example\ntype: local\nconfig: "+fixture.Input), &target)
			if expected, ok := fixture.Target["Ok"]; ok {
				if err != nil {
					t.Fatal(err)
				}
				encoded, err = json.Marshal(map[string]any{"config": target.Config})
				if err != nil {
					t.Fatal(err)
				}
				compare(t, encoded, expected)
			} else if err == nil {
				t.Fatal("unsupported JSON config accepted")
			}
			var index Index
			if err := yaml.Unmarshal([]byte("extension: "+fixture.Input), &index); err != nil {
				t.Fatal(err)
			}
			encoded, err = json.Marshal(index.Extra[0].Value)
			if err != nil {
				t.Fatal(err)
			}
			compare(t, encoded, fixture.Index)
		})
	}
}

func TestYAMLMergeKeysAreMetadataRatherThanImplicitInheritance(t *testing.T) {
	var config MindConfig
	if err := yaml.Unmarshal([]byte("<<: {name: inherited, build: {format: html}}\n"), &config); err != nil {
		t.Fatal(err)
	}
	if config.Name != nil || config.Build.Format != "md" {
		t.Fatalf("merge expanded: %#v", config)
	}
	var source Source
	if err := yaml.Unmarshal([]byte("name: explicit\n<<: {name: inherited}\n7: value\n"), &source); err != nil {
		t.Fatal(err)
	}
	raw, err := json.Marshal(source)
	if err != nil {
		t.Fatal(err)
	}
	var object map[string]any
	if err := json.Unmarshal(raw, &object); err != nil {
		t.Fatal(err)
	}
	if object["7"] != "value" || object["<<"].(map[string]any)["name"] != "inherited" || source.Name != "explicit" {
		t.Fatal(object)
	}
	if raw, err := yaml.Marshal(source); err != nil || len(raw) == 0 {
		t.Fatalf("metadata YAML roundtrip: %s (%v)", raw, err)
	}
}

func TestNonfiniteConfidenceSerializesAsPresentNull(t *testing.T) {
	for _, value := range []float64{math.NaN(), math.Inf(1), math.Inf(-1)} {
		for _, object := range []any{Term{Confidence: &value}, TermFinding{Confidence: &value}, CandidateTerm{Confidence: &value}} {
			raw, err := json.Marshal(object)
			if err != nil {
				t.Fatal(err)
			}
			var fields map[string]json.RawMessage
			if err := json.Unmarshal(raw, &fields); err != nil {
				t.Fatal(err)
			}
			if string(fields["confidence"]) != "null" {
				t.Fatalf("%T confidence=%s", object, fields["confidence"])
			}
		}
	}
}

func TestRequiredAndNonnullableFieldsRejectNullAliases(t *testing.T) {
	for _, input := range []string{"empty: &empty null\nbuild: *empty\n", "empty: &empty null\nreserved_roots: *empty\n"} {
		var config MindConfig
		if err := yaml.Unmarshal([]byte(input), &config); err == nil {
			t.Fatalf("null alias accepted: %s", input)
		}
	}
	var source Source
	if err := yaml.Unmarshal([]byte("empty: &empty null\nname: *empty\n"), &source); err == nil {
		t.Fatal("required string accepted null alias")
	}
}

func TestYAMLMetadataAliasesSurviveKnownFieldReencoding(t *testing.T) {
	var source Source
	if err := yaml.Unmarshal([]byte("name: &name example\nmetadata: *name\n"), &source); err != nil {
		t.Fatal(err)
	}
	raw, err := yaml.Marshal(source)
	if err != nil {
		t.Fatal(err)
	}
	var reloaded Source
	if err := yaml.Unmarshal(raw, &reloaded); err != nil {
		t.Fatalf("reencoded metadata contains a dangling alias: %s (%v)", raw, err)
	}
	encoded, err := json.Marshal(reloaded)
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]any
	if err := json.Unmarshal(encoded, &fields); err != nil || fields["metadata"] != "example" {
		t.Fatalf("alias value lost: %s (%v)", encoded, err)
	}
	var config MindConfig
	if err := yaml.Unmarshal([]byte("name: &name example\narticles: {reference: *name}\n"), &config); err != nil {
		t.Fatal(err)
	}
	raw, err = yaml.Marshal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := yaml.Unmarshal(raw, &config); err != nil {
		t.Fatalf("raw article metadata alias lost: %s (%v)", raw, err)
	}
}

func TestFloatFormattingMatchesFrozenSerdeJSONCorpus(t *testing.T) {
	raw, err := os.ReadFile("testdata/float-legacy.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures [][2]string
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	for _, fixture := range fixtures {
		bits, err := strconv.ParseUint(fixture[0], 16, 64)
		if err != nil {
			t.Fatal(err)
		}
		if actual := serdeFloat(math.Float64frombits(bits)); actual != fixture[1] {
			t.Fatalf("float %s=%s, want %s", fixture[0], actual, fixture[1])
		}
	}
}

func TestOptionalConfidenceAndTaggedRawArticlesKeepPresence(t *testing.T) {
	for _, value := range []any{Term{}, TermFinding{}, CandidateTerm{}} {
		raw, err := json.Marshal(value)
		if err != nil || strings.Contains(string(raw), `"confidence"`) {
			t.Fatalf("absent confidence serialized: %s (%v)", raw, err)
		}
	}
	zero := 0.0
	raw, err := json.Marshal(Term{Confidence: &zero})
	if err != nil || !strings.Contains(string(raw), `"confidence":0.0`) {
		t.Fatalf("present float zero lost: %s (%v)", raw, err)
	}
	var config MindConfig
	if err := yaml.Unmarshal([]byte("articles: !memo 7\n"), &config); err != nil {
		t.Fatal(err)
	}
	raw, err = json.Marshal(config)
	if err != nil || !strings.Contains(string(raw), `"articles":{"!memo":7}`) {
		t.Fatalf("raw tag serialization: %s (%v)", raw, err)
	}
	for _, value := range []string{"null", ".nan"} {
		var target PublishTarget
		if err := yaml.Unmarshal([]byte("name: local\ntype: local\nconfig: "+value), &target); err != nil {
			t.Fatal(err)
		}
		raw, err = json.Marshal(target)
		if err != nil {
			t.Fatal(err)
		}
		present := strings.Contains(string(raw), `"config":null`)
		if present != (value == ".nan") {
			t.Fatalf("None versus Some(null) lost: %s => %s", value, raw)
		}
	}
}
