package term

import (
	"fmt"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestMissingTermDistinguishesMissingCorrection(t *testing.T) {
	_, absent := findTerm(nil, "RAG")
	_, correction := findCorrection(model.Term{Term: "RAG"}, "global-only")
	for _, c := range []struct {
		name string
		err  error
		want bool
	}{
		{"canonical", absent, true}, {"wrapped canonical", fmt.Errorf("read term: %w", absent), true},
		{"correction", correction, false}, {"wrapped correction", fmt.Errorf("patch: %w", correction), false},
		{"other not found", &output.Error{Kind: "not_found", Message: "missing file"}, false}, {"nil", nil, false},
	} {
		if got := IsMissingTerm(c.err); got != c.want {
			t.Errorf("%s: missing term=%v want %v", c.name, got, c.want)
		}
	}
}
