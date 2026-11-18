package lifecycle

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

type Entry struct {
	Kind  string
	Bytes []byte
	Mode  uint32
}

type Snapshot map[string]Entry

type Plan struct {
	Version   uint32   `json:"version"`
	Root      string   `json:"root"`
	Operation string   `json:"operation"`
	Watches   []string `json:"watches"`
	Before    Snapshot `json:"before"`
	After     Snapshot `json:"after"`
	Digest    string   `json:"digest"`
}

type Journal struct {
	Plan  Plan   `json:"plan"`
	Phase string `json:"phase"`
}

func (entry Entry) MarshalJSON() ([]byte, error) {
	return CanonicalJSON(entry)
}

func (entry *Entry) UnmarshalJSON(raw []byte) error {
	fields, err := protocolFields(raw, "kind", "mode")
	if err != nil {
		return err
	}
	var result Entry
	for _, field := range []struct {
		name   string
		target any
	}{{"kind", &result.Kind}, {"mode", &result.Mode}} {
		value, exists := fields[field.name]
		if !exists || bytes.Equal(value, []byte("null")) {
			return fmt.Errorf("missing field %q", field.name)
		}
		if err := json.Unmarshal(value, field.target); err != nil {
			return err
		}
	}
	switch result.Kind {
	case "file":
		if _, err := protocolFields(raw, "kind", "mode", "bytes"); err != nil {
			return err
		}
		value := bytes.TrimSpace(fields["bytes"])
		if len(value) == 0 || value[0] != '[' {
			return fmt.Errorf("file bytes must be a numeric array")
		}
		var elements []json.RawMessage
		if err := json.Unmarshal(value, &elements); err != nil {
			return err
		}
		result.Bytes = make([]byte, len(elements))
		for index, element := range elements {
			if bytes.Equal(bytes.TrimSpace(element), []byte("null")) {
				return fmt.Errorf("file bytes must contain integers")
			}
			if err := json.Unmarshal(element, &result.Bytes[index]); err != nil {
				return err
			}
		}
	case "directory":
	default:
		return fmt.Errorf("unknown entry kind %q", result.Kind)
	}
	*entry = result
	return nil
}

func protocolFields(raw []byte, known ...string) (map[string]json.RawMessage, error) {
	if err := validateProtocolJSON(raw); err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	start, err := decoder.Token()
	if err != nil {
		return nil, err
	}
	if start != json.Delim('{') {
		return nil, fmt.Errorf("expected lifecycle object")
	}
	fields := make(map[string]json.RawMessage)
	for decoder.More() {
		token, err := decoder.Token()
		if err != nil {
			return nil, err
		}
		name, ok := token.(string)
		if !ok {
			return nil, fmt.Errorf("expected string field name")
		}
		if _, exists := fields[name]; exists {
			for _, field := range known {
				if field == name {
					return nil, fmt.Errorf("duplicate field %q", name)
				}
			}
		}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return nil, err
		}
		fields[name] = bytes.TrimSpace(value)
	}
	if _, err := decoder.Token(); err != nil {
		return nil, err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return nil, fmt.Errorf("unexpected trailing lifecycle data")
	}
	return fields, nil
}

// encoding/json replaces malformed Unicode with U+FFFD. Lifecycle inputs must
// instead reject it, so a saved path or digest is never silently rewritten.
func validateProtocolJSON(raw []byte) error {
	if !utf8.Valid(raw) || !json.Valid(raw) {
		return fmt.Errorf("invalid lifecycle JSON")
	}
	inside := false
	for index := 0; index < len(raw); index++ {
		if !inside {
			if raw[index] == '"' {
				inside = true
			}
			continue
		}
		if raw[index] == '"' {
			inside = false
			continue
		}
		if raw[index] != '\\' {
			continue
		}
		index++
		if raw[index] != 'u' {
			continue
		}
		value, _ := strconv.ParseUint(string(raw[index+1:index+5]), 16, 16)
		if value >= 0xdc00 && value <= 0xdfff {
			return fmt.Errorf("lone low surrogate in lifecycle JSON")
		}
		if value >= 0xd800 && value <= 0xdbff {
			if index+11 > len(raw) || raw[index+5] != '\\' || raw[index+6] != 'u' {
				return fmt.Errorf("lone high surrogate in lifecycle JSON")
			}
			low, err := strconv.ParseUint(string(raw[index+7:index+11]), 16, 16)
			if err != nil || low < 0xdc00 || low > 0xdfff {
				return fmt.Errorf("invalid surrogate pair in lifecycle JSON")
			}
			index += 10
		} else {
			index += 4
		}
	}
	return nil
}

func (plan *Plan) UnmarshalJSON(raw []byte) error {
	names := []string{"version", "root", "operation", "watches", "before", "after", "digest"}
	fields, err := protocolFields(raw, names...)
	if err != nil {
		return err
	}
	for _, name := range names {
		if value := fields[name]; len(value) == 0 || bytes.Equal(value, []byte("null")) {
			return fmt.Errorf("missing field %q", name)
		}
	}
	type wire Plan
	var result wire
	if err := json.Unmarshal(raw, &result); err != nil {
		return err
	}
	var watches []json.RawMessage
	if err := json.Unmarshal(fields["watches"], &watches); err != nil {
		return err
	}
	for _, watch := range watches {
		if bytes.Equal(bytes.TrimSpace(watch), []byte("null")) {
			return fmt.Errorf("watches must contain strings")
		}
	}
	*plan = Plan(result)
	return nil
}

func (journal *Journal) UnmarshalJSON(raw []byte) error {
	fields, err := protocolFields(raw, "plan", "phase")
	if err != nil {
		return err
	}
	for _, name := range []string{"plan", "phase"} {
		if value := fields[name]; len(value) == 0 || bytes.Equal(value, []byte("null")) {
			return fmt.Errorf("missing field %q", name)
		}
	}
	type wire Journal
	var result wire
	if err := json.Unmarshal(raw, &result); err != nil {
		return err
	}
	if result.Phase != "prepared" && result.Phase != "complete" && result.Phase != "rolled_back" {
		return fmt.Errorf("unknown journal phase %q", result.Phase)
	}
	*journal = Journal(result)
	return nil
}

// CanonicalJSON encodes the integer-only lifecycle protocol with serde's
// struct order, sorted snapshot keys, numeric bytes and unescaped UTF-8.
func CanonicalJSON(value any) ([]byte, error) {
	var buffer bytes.Buffer
	if err := encodeCanonical(&buffer, reflect.ValueOf(value)); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func encodeCanonical(buffer *bytes.Buffer, value reflect.Value) error {
	if !value.IsValid() {
		buffer.WriteString("null")
		return nil
	}
	if value.Kind() == reflect.Interface || value.Kind() == reflect.Pointer {
		if value.IsNil() {
			buffer.WriteString("null")
			return nil
		}
		return encodeCanonical(buffer, value.Elem())
	}
	if entry, ok := value.Interface().(Entry); ok {
		buffer.WriteString(`{"kind":`)
		if err := encodeString(buffer, entry.Kind); err != nil {
			return err
		}
		if entry.Kind == "file" {
			buffer.WriteString(`,"bytes":`)
			if err := encodeCanonical(buffer, reflect.ValueOf(entry.Bytes)); err != nil {
				return err
			}
		} else if entry.Kind != "directory" {
			return fmt.Errorf("unknown entry kind %q", entry.Kind)
		}
		buffer.WriteString(`,"mode":`)
		buffer.WriteString(strconv.FormatUint(uint64(entry.Mode), 10))
		buffer.WriteByte('}')
		return nil
	}
	switch value.Kind() {
	case reflect.String:
		return encodeString(buffer, value.String())
	case reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uint:
		buffer.WriteString(strconv.FormatUint(value.Uint(), 10))
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		buffer.WriteString(strconv.FormatInt(value.Int(), 10))
	case reflect.Bool:
		buffer.WriteString(strconv.FormatBool(value.Bool()))
	case reflect.Slice, reflect.Array:
		buffer.WriteByte('[')
		for i := 0; i < value.Len(); i++ {
			if i > 0 {
				buffer.WriteByte(',')
			}
			if err := encodeCanonical(buffer, value.Index(i)); err != nil {
				return err
			}
		}
		buffer.WriteByte(']')
	case reflect.Map:
		if value.Type().Key().Kind() != reflect.String {
			return fmt.Errorf("lifecycle maps require string keys")
		}
		keys := value.MapKeys()
		sort.Slice(keys, func(i, j int) bool { return keys[i].String() < keys[j].String() })
		buffer.WriteByte('{')
		for i, key := range keys {
			if i > 0 {
				buffer.WriteByte(',')
			}
			if err := encodeString(buffer, key.String()); err != nil {
				return err
			}
			buffer.WriteByte(':')
			if err := encodeCanonical(buffer, value.MapIndex(key)); err != nil {
				return err
			}
		}
		buffer.WriteByte('}')
	case reflect.Struct:
		buffer.WriteByte('{')
		for i := 0; i < value.NumField(); i++ {
			field := value.Type().Field(i)
			name := strings.Split(field.Tag.Get("json"), ",")[0]
			if name == "" || name == "-" || !field.IsExported() {
				return fmt.Errorf("unsupported lifecycle field %s", field.Name)
			}
			if i > 0 {
				buffer.WriteByte(',')
			}
			if err := encodeString(buffer, name); err != nil {
				return err
			}
			buffer.WriteByte(':')
			if err := encodeCanonical(buffer, value.Field(i)); err != nil {
				return err
			}
		}
		buffer.WriteByte('}')
	default:
		return fmt.Errorf("unsupported lifecycle value %s", value.Kind())
	}
	return nil
}

func encodeString(buffer *bytes.Buffer, value string) error {
	if !utf8.ValidString(value) {
		return fmt.Errorf("lifecycle string is not valid UTF-8")
	}
	const digits = "0123456789abcdef"
	buffer.WriteByte('"')
	for _, char := range value {
		switch char {
		case '"', '\\':
			buffer.WriteByte('\\')
			buffer.WriteRune(char)
		case '\b':
			buffer.WriteString(`\b`)
		case '\f':
			buffer.WriteString(`\f`)
		case '\n':
			buffer.WriteString(`\n`)
		case '\r':
			buffer.WriteString(`\r`)
		case '\t':
			buffer.WriteString(`\t`)
		default:
			if char < 0x20 {
				buffer.WriteString(`\u00`)
				buffer.WriteByte(digits[char>>4])
				buffer.WriteByte(digits[char&15])
			} else {
				buffer.WriteRune(char)
			}
		}
	}
	buffer.WriteByte('"')
	return nil
}

func Digest(raw []byte) string {
	hash := sha256.Sum256(raw)
	return hex.EncodeToString(hash[:])
}

func (entry Entry) Hash() (string, error) {
	raw, err := CanonicalJSON(entry)
	if err != nil {
		return "", err
	}
	return Digest(raw), nil
}

func (plan Plan) ComputeDigest() (string, error) {
	raw, err := CanonicalJSON([]any{plan.Version, plan.Root, plan.Operation, plan.Watches, plan.Before, plan.After})
	if err != nil {
		return "", err
	}
	return Digest(raw), nil
}
