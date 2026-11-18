package main

import (
	"fmt"
	"io"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/outcome"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
)

type commandExit struct{ code int }

func (status *commandExit) Error() string {
	return fmt.Sprintf("command completed with exit %d", status.code)
}

func jsonObject(value any) (map[string]any, error) { return outcome.Object(value) }

func renderMindText(data map[string]any, application *app.Context, stdout io.Writer) error {
	if _, created := data["created_at"]; created {
		prefix := "✓ created"
		if data["dry_run"] == true {
			prefix = "would create"
		}
		return output.RenderRaw(stdout, "text", fmt.Sprintf("%s %s: %s", prefix, data["kind"], data["identity"]))
	}
	native, err := jsonObject(data)
	if err != nil {
		return err
	}
	for _, plural := range []string{"minds", "outputs"} {
		if items, ok := native[plural].([]any); ok {
			view := output.List{Headers: []string{"IDENTITY", "PATH"}, Plural: plural}
			for _, raw := range items {
				item, _ := raw.(map[string]any)
				identity, _ := item["identity"].(string)
				path, ok := item["path"].(string)
				if !ok {
					path = "-"
				}
				view.Rows = append(view.Rows, output.TextCells(identity, path))
			}
			options := output.ListOptionsFromProfile(commandProfile(stdout, application), false, false)
			return output.RenderRaw(stdout, "text", output.RenderList(view, options))
		}
	}
	keys := make([]string, 0, len(native))
	for key := range native {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	lines := make([]string, 0, len(keys))
	for _, key := range keys {
		value, ok := native[key].(string)
		if !ok {
			raw, err := output.CompactJSON(native[key])
			if err != nil {
				return err
			}
			value = string(raw)
		}
		lines = append(lines, key+":  "+value)
	}
	return output.RenderRaw(stdout, "text", strings.Join(lines, "\n"))
}
