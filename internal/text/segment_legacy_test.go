package text

import (
	"reflect"
	"testing"
)

func TestLegacySegmentationBoundaries(t *testing.T) {
	t.Run("segment_boundary_set", func(t *testing.T) {
		s := Segment("实现理想的精神")
		if !s.boundaries[0] || !reflect.DeepEqual(Cut("实现理想的精神"), []string{"实现理想", "的", "精神"}) {
			t.Fatalf("initial boundary or HMM tokens changed: %+v", s)
		}
	})
	t.Run("standalone_word_spans_align", func(t *testing.T) {
		if !Segment("时刻").Aligned(0, len("时刻")) {
			t.Fatal("standalone word does not align")
		}
	})
	t.Run("span_aligns_for_full_token", func(t *testing.T) {
		s := Segment("时刻")
		if !s.Aligned(0, len("时刻")) || s.boundaries[len("时")] {
			t.Fatal("full word boundary split inside token")
		}
	})
	t.Run("span_misaligns_for_cross_boundary", func(t *testing.T) {
		tokens := Cut("缩小文件")
		found := false
		for _, token := range tokens {
			if token == "缩小" {
				found = true
			}
		}
		if !found || Segment("缩小文件").Aligned(len("缩"), len("小文")) {
			t.Fatalf("cross-token substring accepted or expected token absent: %q", tokens)
		}
	})
	t.Run("edge_of_document_is_boundary", func(t *testing.T) {
		s := Segment("时刻")
		if !s.boundaries[0] || !s.boundaries[len("时刻")] {
			t.Fatal("document edge missing from boundaries")
		}
	})
	t.Run("empty_document", func(t *testing.T) {
		s := Segment("")
		if !s.boundaries[0] || len(s.boundaries) != 1 || len(s.exact) != 0 {
			t.Fatalf("empty document boundary/token sets: %+v", s)
		}
	})
	t.Run("is_token_true_for_full_token", func(t *testing.T) {
		if !Segment("时刻").Exact(0, len("时刻")) {
			t.Fatal("full word is not an exact token")
		}
	})
	t.Run("is_token_false_for_cross_token_span", func(t *testing.T) {
		if Segment("缩小文件").Exact(len("缩"), len("小文")) {
			t.Fatal("cross-token substring accepted as exact token")
		}
	})
	t.Run("is_token_false_for_adjacent_token_pair", func(t *testing.T) {
		if Segment("以可独立验证和回退的方案").Exact(0, len("以可")) {
			t.Fatal("two adjacent tokens accepted as one exact token")
		}
	})
	t.Run("shared_instance_is_deterministic_across_calls", func(t *testing.T) {
		first := Segment("实现理想的精神")
		second := Segment("实现理想的精神")
		if !reflect.DeepEqual(first.boundaries, second.boundaries) {
			t.Fatal("repeated segmentation changed boundaries")
		}
		_ = Segment("缩小文件")
		third := Segment("实现理想的精神")
		if !reflect.DeepEqual(first.boundaries, third.boundaries) {
			t.Fatal("another document perturbed shared model")
		}
	})
}
