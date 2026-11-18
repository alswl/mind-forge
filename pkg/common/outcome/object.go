package outcome

import (
	"bytes"
	"encoding/json"
)

func Object(value any) (map[string]any, error) {
	raw, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var data map[string]any
	err = decoder.Decode(&data)
	return data, err
}
