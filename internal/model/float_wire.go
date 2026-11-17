package model

import "encoding/json"

func optionalFloat(value *float64) any {
	if value == nil {
		return nil
	}
	return json.RawMessage(serdeFloat(*value))
}

func (term Term) MarshalJSON() ([]byte, error) {
	type wire Term
	return json.Marshal(struct {
		wire
		Confidence any `json:"confidence,omitempty"`
	}{wire(term), optionalFloat(term.Confidence)})
}

func (finding TermFinding) MarshalJSON() ([]byte, error) {
	type wire TermFinding
	return json.Marshal(struct {
		wire
		Confidence any `json:"confidence,omitempty"`
	}{wire(finding), optionalFloat(finding.Confidence)})
}

func (candidate CandidateTerm) MarshalJSON() ([]byte, error) {
	type wire CandidateTerm
	return json.Marshal(struct {
		wire
		Confidence any `json:"confidence,omitempty"`
	}{wire(candidate), optionalFloat(candidate.Confidence)})
}
