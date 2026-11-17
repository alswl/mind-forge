package output

import (
	"bytes"
	"errors"
	"io"
	"strings"
	"testing"
)

func TestLegacyStripANSI(t *testing.T) {
	if got := stripANSI("\x1b[31mhello\x1b[0m world"); got != "hello world" {
		t.Fatal(got)
	}
}
func TestLegacyStripANSIPlain(t *testing.T) {
	if got := stripANSI("hello world"); got != "hello world" {
		t.Fatal(got)
	}
}
func TestLegacyDeprecationSubject(t *testing.T) {
	var buf bytes.Buffer
	if err := WarnDeprecation(&buf, "--status", "--set status=<value>", false); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(buf.String(), "[deprecated] --status is deprecated, use --set status=<value> instead") {
		t.Fatal(buf.String())
	}
}
func TestLegacyDeprecationNoColor(t *testing.T) {
	var buf bytes.Buffer
	if err := WarnDeprecation(&buf, "\x1b[31m--status\x1b[0m", "--set status=<value>", true); err != nil {
		t.Fatal(err)
	}
	if strings.ContainsRune(buf.String(), '\x1b') || !strings.Contains(buf.String(), "[deprecated] --status is deprecated") {
		t.Fatal(buf.String())
	}
}
func TestDeprecationWriterFailure(t *testing.T) {
	if err := WarnDeprecation(failingWriter{}, "--status", "--set status=<value>", false); !errors.Is(err, io.ErrClosedPipe) {
		t.Fatal(err)
	}
}
