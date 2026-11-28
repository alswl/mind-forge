package output

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"unicode/utf8"
)

func RenderRaw(writer io.Writer, format, content string) error {
	if format != "json" {
		_, err := fmt.Fprintln(writer, content)
		return err
	}
	var data any
	decoder := json.NewDecoder(strings.NewReader(content))
	decoder.UseNumber()
	if err := decoder.Decode(&data); err != nil {
		data = content
	} else {
		var trailing any
		if err := decoder.Decode(&trailing); err != io.EOF {
			data = content
		}
	}
	raw, err := marshalLine(map[string]any{"status": "ok", "command": "mf", "data": data})
	if err != nil {
		return err
	}
	_, err = writer.Write(raw)
	return err
}

func RenderError(writer io.Writer, format string, failure *Error) error {
	return RenderNamedError(writer, format, "mf", failure)
}

func RenderNamedError(writer io.Writer, format, command string, failure *Error) error {
	if format == "json" {
		_, err := writer.Write(failure.NamedJSON(command))
		return err
	}
	var text strings.Builder
	fmt.Fprintf(&text, "error: %s\n", failure.Message)
	if failure.Hint != nil {
		fmt.Fprintf(&text, "\nHint: %s\n", *failure.Hint)
	}
	fmt.Fprintf(&text, "\nRun `%s --help` for usage.\n", command)
	_, err := io.WriteString(writer, text.String())
	return err
}

func RenderSuccess(writer io.Writer, format string, data any) error {
	if format == "json" {
		raw, err := Success("mf", data)
		if err != nil {
			return err
		}
		_, err = writer.Write(raw)
		return err
	}
	raw, err := json.Marshal(data)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		return err
	}
	var text strings.Builder
	switch value := value.(type) {
	case string:
		fmt.Fprintln(&text, value)
	case map[string]any:
		keys := make([]string, 0, len(value))
		width := 0
		for key := range value {
			keys = append(keys, key)
			if len(key) > width {
				width = len(key)
			}
		}
		sort.Strings(keys)
		for _, key := range keys {
			content, ok := value[key].(string)
			if !ok {
				encoded, err := marshalLine(value[key])
				if err != nil {
					return err
				}
				content = strings.TrimSuffix(string(encoded), "\n")
			}
			fmt.Fprintf(&text, "%s%s  %s\n", strings.Repeat(" ", width-utf8.RuneCountInString(key)), key, content)
		}
	default:
		text.Write(raw)
		text.WriteByte('\n')
	}
	_, err = io.WriteString(writer, text.String())
	return err
}

func EmitWarning(writer io.Writer, warnings *[]string, message string) {
	fmt.Fprintf(writer, "WARN: %s\n", message)
	*warnings = append(*warnings, message)
}
