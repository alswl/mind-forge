package term

import (
	"fmt"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func (s Scope) QueryCorrections(name string) (model.Term, error) {
	terms, err := s.Load(true)
	if err != nil {
		return model.Term{}, err
	}
	for _, tm := range terms {
		if tm.Term == name {
			return tm, nil
		}
	}
	return model.Term{}, &output.Error{Kind: "not_found", Message: "term '" + name + "' not found"}
}

func (s Scope) QueryCorrection(name, original string) (model.Correction, error) {
	tm, err := s.QueryCorrections(name)
	if err != nil {
		return model.Correction{}, err
	}
	for _, correction := range tm.Corrections {
		if correction.Original == original {
			return correction, nil
		}
	}
	return model.Correction{}, &output.Error{Kind: "not_found", Message: fmt.Sprintf(`correction "%s" not found on term "%s"`, original, name)}
}
