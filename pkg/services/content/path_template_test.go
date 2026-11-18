package content

import (
	"errors"
	"reflect"
	"testing"
	"time"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestPathTemplateParseRetainsLiteralAndDateSegments(t *testing.T) {
	for _, test := range []struct {
		pattern  string
		segments []PathSegment
	}{
		{"/tmp/output/", []PathSegment{{Literal: "/tmp/output/"}}},
		{"{date:YYYY}", []PathSegment{{DateFormat: "YYYY"}}},
		{"{date:YYYY-MM}", []PathSegment{{DateFormat: "YYYY-MM"}}},
		{"{date:YYYY-MM-DD}", []PathSegment{{DateFormat: "YYYY-MM-DD"}}},
		{"/reports/{date:YYYY-MM}/daily/", []PathSegment{{Literal: "/reports/"}, {DateFormat: "YYYY-MM"}, {Literal: "/daily/"}}},
		{"", []PathSegment{}},
	} {
		t.Run(test.pattern, func(t *testing.T) {
			template, err := ParsePathTemplate(test.pattern)
			if err != nil || !reflect.DeepEqual(template.Segments, test.segments) {
				t.Fatalf("%#v %v", template, err)
			}
		})
	}
	for _, pattern := range []string{"{quarter:QQ}", "/reports/{date:YYYY", "{lang:en}", "{lang:en}/{date:YYYY-MM-DD}.md", "{date:YYYY/MM/DD}"} {
		_, err := ParsePathTemplate(pattern)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "unknown_placeholder" {
			t.Fatalf("%s: %v", pattern, err)
		}
	}
}

func TestPathTemplateExpandFrozenDateForms(t *testing.T) {
	date := time.Date(2026, 5, 15, 0, 0, 0, 0, time.UTC)
	for _, test := range []struct{ pattern, want string }{
		{"{date:YYYY}", "2026"}, {"{date:YYYY-MM}", "2026-05"}, {"{date:YYYY-MM-DD}", "2026-05-15"},
		{"/reports/{date:YYYY-MM}/daily/", "/reports/2026-05/daily/"},
		{"outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md", "outputs/2026-05/2026-05-15.md"},
	} {
		template, err := ParsePathTemplate(test.pattern)
		if err != nil {
			t.Fatal(err)
		}
		if got := template.Expand(date); got != test.want {
			t.Fatalf("%s => %s want%s", test.pattern, got, test.want)
		}
	}
}

func TestPathTemplateMatcherRetainsEverySlotAndMostSpecificFormat(t *testing.T) {
	for _, test := range []struct {
		pattern, path, slot, format string
		count                       int
	}{
		{"outputs/{date:YYYY-MM-DD}.md", "outputs/2026-05-15.md", "2026-05-15", "YYYY-MM-DD", 1},
		{"outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md", "outputs/2026-05/2026-05-15.md", "2026-05-15", "YYYY-MM-DD", 2},
	} {
		template, err := ParsePathTemplate(test.pattern)
		if err != nil {
			t.Fatal(err)
		}
		got, ok := template.Match(test.path)
		if !ok || got.MostSpecificValue != test.slot || len(got.Slots) != test.count || template.MostSpecificFormat() != test.format {
			t.Fatalf("%#v %v", got, ok)
		}
		if _, ok := template.Match("other/2026-05-15.md"); ok {
			t.Fatal("matched foreign prefix")
		}
	}
	template, _ := ParsePathTemplate("{date:YYYY-MM-DD}.md")
	if got, ok := template.Match("2026-99-99.md"); !ok || got.MostSpecificValue != "2026-99-99" {
		t.Fatal("matcher must retain syntactic capture; date validity is a separate decision")
	}
	for _, path := range []string{"2026-05-15.md.extra", "2026-aa-15.md", "2026-05.md"} {
		if _, ok := template.Match(path); ok {
			t.Fatalf("matched %s", path)
		}
	}
}

func TestPathTemplateValidatesSlotsAfterParsing(t *testing.T) {
	for _, pattern := range []string{"{date:YYYY-MM-DD}.md", "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md"} {
		template, err := ParsePathTemplate(pattern)
		if err != nil || template.ValidateSlots("reports") != nil {
			t.Fatalf("%s %v", pattern, err)
		}
	}
	template, err := ParsePathTemplate("{date:YYYY}/{date:YYYY}.md")
	if err != nil {
		t.Fatal("parse must not perform slot redundancy validation")
	}
	var failure *output.Error
	if err := template.ValidateSlots("reports"); !errors.As(err, &failure) || failure.Kind != "multi_slot_template" {
		t.Fatalf("%v", err)
	}
}
