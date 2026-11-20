package output

import (
	"encoding/json"
	"fmt"
	"reflect"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/outcome"
)

type Verb = outcome.Verb
type VerbResult = outcome.VerbResult

const (
	Create = outcome.Create
	Add    = outcome.Add
	Rename = outcome.Rename
	Remove = outcome.Remove
	Update = outcome.Update
	Move   = outcome.Move
	Index  = outcome.Index
	Lint   = outcome.Lint
)

func textField(object map[string]any, key, fallback string) string {
	if value, ok := object[key].(string); ok {
		return value
	}
	return fallback
}

func countField(object map[string]any, key string) uint64 {
	switch value := object[key].(type) {
	case uint64:
		return value
	case uint:
		return uint64(value)
	case int:
		if value >= 0 {
			return uint64(value)
		}
	case int64:
		if value >= 0 {
			return uint64(value)
		}
	case json.Number:
		parsed, err := strconv.ParseUint(string(value), 10, 64)
		if err == nil {
			return parsed
		}
	}
	return 0
}

func arrayLength(value any) int {
	if value == nil {
		return 0
	}
	reflected := reflect.ValueOf(value)
	if reflected.Kind() == reflect.Slice || reflected.Kind() == reflect.Array {
		return reflected.Len()
	}
	return 0
}

func RenderVerb(result VerbResult, options DisplayOptions) string {
	old := "?"
	if result.OldIdentity != nil {
		old = *result.OldIdentity
	}
	prefix, action := "✓ ", ""
	if result.DryRun {
		prefix = "[dry-run] would "
	}
	switch result.Verb {
	case Create:
		action = "created"
		if result.DryRun {
			action = "create"
		}
	case Add:
		action = "added"
		if result.DryRun {
			action = "add"
		}
	case Remove:
		action = "removed"
		if result.DryRun {
			action = "remove"
		}
	case Rename:
		action = "renamed"
		if result.DryRun {
			action = "rename"
		}
		destination := ""
		if result.DryRun && result.Kind == "source" && result.Path != nil {
			destination = " (" + *result.Path + ")"
		}
		return fmt.Sprintf("%s%s %s: %s → %s%s", prefix, action, result.Kind, old, result.Identity, destination)
	case Update:
		action = "updated"
		if result.DryRun {
			action = "update"
		}
		changes, _ := result.Details["changes"].(map[string]any)
		plural := "s"
		if len(changes) == 1 {
			plural = ""
		}
		return fmt.Sprintf("%s%s %s: %s (%d field%s)", prefix, action, result.Kind, result.Identity, len(changes), plural)
	case Move:
		action = "moved"
		if result.DryRun {
			action = "move"
		}
		return fmt.Sprintf("%s%s %s: %s (%s → %s)", prefix, action, result.Kind, result.Identity, textField(result.Details, "from_scope", "?"), textField(result.Details, "to_scope", "?"))
	case Index:
		prefix = "indexed"
		if result.DryRun {
			prefix = "[dry-run] would index"
		}
		added := arrayLength(result.Details["added"])
		kept := countField(result.Details, "kept_count")
		removed := arrayLength(result.Details["removed"])
		if result.Kind == "source" {
			return fmt.Sprintf("%s source: added %d, kept %d, missing %d (retained)", prefix, added, kept, removed)
		}
		return fmt.Sprintf("%s %s: +%d =%d -%d", prefix, result.Kind, added, kept, removed)
	case Lint:
		return renderLint(result, options)
	}
	return fmt.Sprintf("%s%s %s: %s", prefix, action, result.Kind, result.Identity)
}

func renderLint(result VerbResult, options DisplayOptions) string {
	var text strings.Builder
	if issues, ok := result.Details["issues"].([]any); ok {
		for _, value := range issues {
			issue, ok := value.(map[string]any)
			if !ok {
				issue = map[string]any{}
			}
			path := RenderPathLink(textField(issue, "path", ""), options)
			if line := countField(issue, "line"); line > 0 {
				path += fmt.Sprintf(":%d", line)
			}
			fmt.Fprintf(&text, "[%s] %s: %s (%s)\n", textField(issue, "severity", "unknown"), textField(issue, "kind", "unknown"), textField(issue, "message", ""), path)
		}
	}
	if summary, ok := result.Details["summary"].(map[string]any); ok {
		fmt.Fprintf(&text, "%d errors, %d warnings, %d info", countField(summary, "errors"), countField(summary, "warnings"), countField(summary, "info"))
		if fixed := countField(summary, "fixed"); fixed > 0 {
			fmt.Fprintf(&text, ", %d fixed", fixed)
		}
		text.WriteByte('\n')
	}
	return text.String()
}

func VerbEnvelope(result VerbResult) map[string]any {
	envelope := map[string]any{"kind": result.Kind, "identity": result.Identity, "dry_run": result.DryRun}
	switch result.Verb {
	case Rename:
		old := ""
		if result.OldIdentity != nil {
			old = *result.OldIdentity
		}
		envelope["old_identity"], envelope["new_identity"] = old, result.Identity
	case Remove:
		envelope["removed"] = true
	case Move:
		for _, key := range []string{"old_path", "new_path", "moved_prompts", "moved_thinking", "moved_publish_records"} {
			if value, ok := result.Details[key]; ok {
				envelope[key] = value
			}
		}
	}
	if result.Path != nil {
		envelope["path"] = *result.Path
	}
	if result.Details != nil {
		if result.Verb == Index || result.Verb == Lint {
			for key, value := range result.Details {
				if _, exists := envelope[key]; !exists {
					envelope[key] = value
				}
			}
		} else {
			envelope["details"] = result.Details
		}
	}
	return envelope
}
