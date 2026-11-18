package source

import (
	"errors"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func TestInputClassificationRetainsExactHTTPPrefixRules(t *testing.T) {
	for _, value := range []string{"http://example.test", "https://example.test/x", "http://", "https:///x"} {
		if ClassifyInput(value) != URLInput {
			t.Fatalf("URL classified as file: %q", value)
		}
	}
	for _, value := range []string{"", "HTTP://example.test", "Https://example.test", " https://example.test", "ftp://example.test/file", "file:///a.md", "sources/机器.md", "C:\\file.pdf"} {
		if ClassifyInput(value) != PathInput {
			t.Fatalf("file classified as URL: %q", value)
		}
	}
}

func TestURLRegistrationValidationIsSeparateFromAcquisitionParsing(t *testing.T) {
	for _, value := range []string{"http://example.test", "https://example.test/x", "https:///x", "http:// "} {
		if err := ValidateURL(value); err != nil {
			t.Fatalf("registration syntax changed for %q: %v", value, err)
		}
	}
	for _, value := range []string{"", "http://", "https://", "ftp://example.test", "HTTPS://example.test"} {
		err := ValidateURL(value)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "usage" || !strings.Contains(failure.Message, "must start with http:// or https:// and include a host") {
			t.Fatalf("invalid URL %q: %v", value, err)
		}
	}
}

func TestNameCollisionHintUsesLongNameFlag(t *testing.T) {
	suggestion := "archive-report"
	for _, test := range []struct {
		suggestion *string
		want       string
	}{{nil, "choose a unique --name"}, {&suggestion, "try --name archive-report"}} {
		err := NameCollisionError("report", test.suggestion)
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Hint == nil || *failure.Hint != test.want || failure.Message != "source name 'report' is already registered" {
			t.Fatalf("wrong collision advice: %#v", err)
		}
	}
}
