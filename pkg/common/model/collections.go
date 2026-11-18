package model

import "encoding/json"

// JSONArray and JSONObject preserve nonnullable collection types on the wire.
// Optional collections use a pointer to distinguish absence from an empty value.
type JSONArray[T any] []T
type JSONObject[T any] map[string]T

func (values JSONArray[T]) MarshalJSON() ([]byte, error) {
	if values == nil {
		return []byte("[]"), nil
	}
	return json.Marshal([]T(values))
}

func (values JSONObject[T]) MarshalJSON() ([]byte, error) {
	if values == nil {
		return []byte("{}"), nil
	}
	return json.Marshal(map[string]T(values))
}
