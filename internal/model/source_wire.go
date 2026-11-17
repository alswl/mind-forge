package model

import "encoding/json"

func (report SourceRemoveReport) MarshalJSON() ([]byte, error) {
	fields, err := jsonFields(report.Source)
	if err != nil {
		return nil, err
	}
	type wire SourceRemoveReport
	outer, err := jsonFields(wire(report))
	if err != nil {
		return nil, err
	}
	for name, value := range outer {
		fields[name] = value
	}
	return json.Marshal(fields)
}
