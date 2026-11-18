package model

import (
	"encoding/json"
	"fmt"
)

type PublishRunOutcome struct {
	Local       *LocalRunOutcome
	YuquePrompt *YuquePromptRunOutcome
}

func (outcome PublishRunOutcome) MarshalJSON() ([]byte, error) {
	if (outcome.Local == nil) == (outcome.YuquePrompt == nil) {
		return nil, fmt.Errorf("publish outcome requires exactly one target variant")
	}
	var body any = outcome.Local
	kind := "local"
	if outcome.YuquePrompt != nil {
		body = outcome.YuquePrompt
		kind = "yuque-prompt"
	}
	fields, err := jsonFields(body)
	if err != nil {
		return nil, err
	}
	tag, _ := json.Marshal(kind)
	fields["target_type"] = tag
	return json.Marshal(fields)
}

func jsonFields(value any) (map[string]json.RawMessage, error) {
	raw, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil {
		return nil, err
	}
	return fields, nil
}
