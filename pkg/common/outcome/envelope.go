package outcome

import (
	"bytes"
	"encoding/json"
	"fmt"
)

func Success(command string, data any) ([]byte, error) {
	raw, err := json.Marshal(data)
	if err != nil {
		return nil, err
	}
	var object map[string]any
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := decoder.Decode(&object); err != nil || object == nil {
		return nil, fmt.Errorf("success data must be an object")
	}
	if _, status := object["status"]; status {
		if _, cmd := object["command"]; cmd {
			if _, body := object["data"]; body {
				return MarshalLine(object)
			}
		}
	}
	return MarshalLine(map[string]any{"status": "ok", "command": command, "data": object})
}

func MarshalLine(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	// encoding/json always escapes Unicode line separators even with HTML
	// escaping disabled. serde leaves those valid UTF-8 characters literal.
	raw := buffer.Bytes()
	result := make([]byte, 0, len(raw))
	for index := 0; index < len(raw); index++ {
		if raw[index] == '\\' && index+1 < len(raw) {
			if index+6 <= len(raw) && (string(raw[index:index+6]) == `\u2028` || string(raw[index:index+6]) == `\u2029`) {
				if raw[index+5] == '8' {
					result = append(result, []byte("\u2028")...)
				} else {
					result = append(result, []byte("\u2029")...)
				}
				index += 5
				continue
			}
			result = append(result, raw[index], raw[index+1])
			index++
			continue
		}
		result = append(result, raw[index])
	}
	return result, nil
}

// CompactJSON keeps the renderer's UTF-8 and escaping rules without a newline.
func CompactJSON(value any) ([]byte, error) {
	raw, err := MarshalLine(value)
	if err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(raw, []byte{'\n'}), nil
}
