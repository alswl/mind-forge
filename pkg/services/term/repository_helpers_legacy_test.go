package term

import (
	"errors"
	"math"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestLegacyTermDedup(t *testing.T) {
	t.Run("dedup_removes_duplicates", func(t *testing.T) {
		if got := dedup([]string{"a", "b", "a", "c"}); !reflect.DeepEqual(got, []string{"a", "b", "c"}) {
			t.Fatalf("dedup = %v", got)
		}
		if got := dedup([]string{"c", "a", "b", "c"}); !reflect.DeepEqual(got, []string{"c", "a", "b"}) {
			t.Fatalf("first occurrence order = %v", got)
		}
	})
	t.Run("dedup_empty", func(t *testing.T) {
		if got := dedup(nil); len(got) != 0 {
			t.Fatalf("empty dedup = %v", got)
		}
	})
}

func TestLegacyConfidenceValidation(t *testing.T) {
	t.Run("confidence_none_is_valid", func(t *testing.T) {
		if err := validateConfidence(nil); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("confidence_boundaries_are_valid", func(t *testing.T) {
		for _, value := range []float64{0, 1, 0.5} {
			if err := validateConfidence(&value); err != nil {
				t.Fatalf("%g: %v", value, err)
			}
		}
	})
	for _, test := range []struct {
		name     string
		value    float64
		fragment string
	}{
		{"confidence_below_range_is_usage_error", -0.1, "out of range"},
		{"confidence_above_range_is_usage_error", 1.1, "out of range"},
		{"confidence_nan_is_usage_error", math.NaN(), "finite"},
		{"confidence_infinity_is_usage_error", math.Inf(1), "finite"},
		{"confidence_neg_infinity_is_usage_error", math.Inf(-1), "finite"},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := validateConfidence(&test.value)
			var failure *output.Error
			if !errors.As(err, &failure) || failure.Kind != "usage" || !strings.Contains(failure.Message, test.fragment) {
				t.Fatalf("confidence %g = %v", test.value, err)
			}
		})
	}
}

func TestLegacyCorrectionLookup(t *testing.T) {
	t.Run("find_correction_index_returns_position", func(t *testing.T) {
		term := model.Term{Term: "RAG", Corrections: []model.Correction{{Original: "a", Correct: "A"}, {Original: "b", Correct: "B"}}}
		for want, original := range []string{"a", "b"} {
			if got, err := findCorrection(term, original); err != nil || got != want {
				t.Fatalf("find %s = %d, %v", original, got, err)
			}
		}
	})
	t.Run("find_correction_index_missing_is_error", func(t *testing.T) {
		term := model.Term{Term: "RAG", Corrections: []model.Correction{{Original: "a", Correct: "A"}}}
		_, err := findCorrection(term, "missing")
		if err == nil || !strings.Contains(err.Error(), "missing") || !strings.Contains(err.Error(), "not found") {
			t.Fatalf("missing correction = %v", err)
		}
	})
}
