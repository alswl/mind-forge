package cli

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

type commandExit struct{ code int }

func (status *commandExit) Error() string {
	return fmt.Sprintf("command completed with exit %d", status.code)
}

func jsonObject(value any) (map[string]any, error) {
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

func mindCommand(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	family := command.Parent().Name()
	var selector *string
	if command.Name() == "show" || family == "output" && (command.Name() == "new" || command.Name() == "list" && len(args) > 0) {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any
	if command.Name() == "index" {
		if boolFlag(command, "check") {
			data, err = content.InspectContent(root, project)
			if err != nil {
				return err
			}
			if !application.Quiet {
				if err := output.RenderSuccess(stdout, application.Format, data); err != nil {
					return err
				}
			}
			if data["consistent"] != true {
				return &commandExit{1}
			}
			return nil
		}
		data, err = content.ReconcileContent(root, project, boolFlag(command, "dry-run"))
	} else if family == "mind" {
		switch command.Name() {
		case "new":
			data, err = content.CreateMind(root, project, args[0], content.MindCreateOptions{Title: optionalFlag(command, "title"), DefaultOutput: optionalFlag(command, "default-output"), DryRun: boolFlag(command, "dry-run")})
		case "list":
			var minds []model.Mind
			minds, err = content.Discover(root, project)
			data = map[string]any{"minds": minds}
		case "show":
			var mind model.Mind
			mind, err = content.ResolveMind(root, project, args[0])
			if err == nil {
				data, err = jsonObject(mind)
			}
		}
	} else {
		switch command.Name() {
		case "new":
			data, err = content.CreateOutput(root, project, args[0], args[1], content.OutputCreateOptions{Title: optionalFlag(command, "title"), File: boolFlag(command, "file"), DryRun: boolFlag(command, "dry-run")})
		case "show":
			var item model.Output
			item, err = content.ResolveOutput(root, project, args[0])
			if err == nil {
				data, err = jsonObject(item)
			}
		case "list":
			var minds []model.Mind
			if len(args) > 0 {
				var mind model.Mind
				mind, err = content.ResolveMind(root, project, args[0])
				minds = []model.Mind{mind}
			} else {
				minds, err = content.Discover(root, project)
			}
			outputs := model.JSONArray[model.Output]{}
			for _, mind := range minds {
				outputs = append(outputs, mind.Outputs...)
			}
			data = map[string]any{"outputs": outputs}
		}
	}
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderMindText(data, application, stdout)
}

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
