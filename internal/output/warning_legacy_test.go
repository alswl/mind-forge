package output

import (
	"bytes"
	"reflect"
	"testing"
)

func TestLegacyWarningCollection(t *testing.T) {
	for _, test := range []struct {
		name     string
		messages []string
	}{
		{"test_single_warning_stderr_and_vec", []string{"test message"}},
		{"test_multiple_warnings_preserve_order", []string{"first", "second", "third"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			var stderr bytes.Buffer
			warnings := []string{}
			for _, message := range test.messages {
				EmitWarning(&stderr, &warnings, message)
			}
			if !reflect.DeepEqual(warnings, test.messages) {
				t.Fatalf("warning collection = %v, want %v", warnings, test.messages)
			}
			want := ""
			for _, message := range test.messages {
				want += "WARN: " + message + "\n"
			}
			if stderr.String() != want {
				t.Fatalf("warning stderr = %q, want %q", stderr.String(), want)
			}
		})
	}
}
