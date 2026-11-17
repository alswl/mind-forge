package model

import (
	"strings"
	"testing"

	"go.yaml.in/yaml/v3"
)

func TestCorrectionEnumErrorsIdentifyFieldAndSequencePosition(t *testing.T) {
	for _, field := range []string{"match", "fix", "boundary"} {
		raw := "term: RAG\ncorrections:\n- {original: rag, correct: RAG}\n- {original: ragg, correct: RAG, " + field + ": bogus}\n"
		var term Term
		err := yaml.Unmarshal([]byte(raw), &term)
		if err == nil || !strings.Contains(err.Error(), "corrections[1]."+field) || !strings.Contains(err.Error(), "bogus") {
			t.Fatalf("%s diagnostic lost field/index/value: %v", field, err)
		}
	}
}
