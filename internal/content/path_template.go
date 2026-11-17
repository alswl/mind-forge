package content

import (
	"fmt"
	"strings"
	"time"

	"github.com/alswl/mind-forge/internal/output"
)

type PathSegment struct{ Literal, DateFormat string }
type PathTemplate struct{ Segments []PathSegment }
type PathDateSlot struct{ Format, Value string }
type PathTemplateMatch struct {
	Slots             []PathDateSlot
	MostSpecificValue string
}

func dateWidth(format string) int {
	switch format {
	case "YYYY":
		return 4
	case "YYYY-MM":
		return 7
	case "YYYY-MM-DD":
		return 10
	}
	return 0
}

func ParsePathTemplate(pattern string) (PathTemplate, error) {
	template := PathTemplate{Segments: []PathSegment{}}
	for pattern != "" {
		at := strings.IndexByte(pattern, '{')
		if at < 0 {
			template.Segments = append(template.Segments, PathSegment{Literal: pattern})
			break
		}
		if at > 0 {
			template.Segments = append(template.Segments, PathSegment{Literal: pattern[:at]})
		}
		pattern = pattern[at:]
		end := strings.IndexByte(pattern, '}')
		token := "{unclosed"
		if end >= 0 {
			token = pattern[:end+1]
		}
		format := ""
		if strings.HasPrefix(token, "{date:") && strings.HasSuffix(token, "}") {
			format = token[len("{date:") : len(token)-1]
		}
		if dateWidth(format) == 0 {
			hint := "supported placeholders: {date:YYYY}, {date:YYYY-MM}, {date:YYYY-MM-DD}"
			return PathTemplate{}, &output.Error{Kind: "unknown_placeholder", Message: fmt.Sprintf("unknown placeholder '%s'", token), Hint: &hint}
		}
		template.Segments = append(template.Segments, PathSegment{DateFormat: format})
		pattern = pattern[end+1:]
	}
	return template, nil
}

func (template PathTemplate) ValidateSlots(name string) error {
	seen := map[string]bool{}
	for _, segment := range template.Segments {
		if segment.DateFormat == "" {
			continue
		}
		if seen[segment.DateFormat] {
			hint := "rewrite template to use a single distinguishing slot; coarse-then-fine date nests are accepted"
			return &output.Error{Kind: "multi_slot_template", Message: fmt.Sprintf("template '%s' has multiple non-redundant slots", name), Hint: &hint}
		}
		seen[segment.DateFormat] = true
	}
	return nil
}

func (template PathTemplate) Expand(date time.Time) string {
	var result strings.Builder
	for _, segment := range template.Segments {
		switch segment.DateFormat {
		case "YYYY":
			result.WriteString(date.Format("2006"))
		case "YYYY-MM":
			result.WriteString(date.Format("2006-01"))
		case "YYYY-MM-DD":
			result.WriteString(date.Format("2006-01-02"))
		default:
			result.WriteString(segment.Literal)
		}
	}
	return result.String()
}

func (template PathTemplate) MostSpecificFormat() string {
	best := ""
	for _, segment := range template.Segments {
		if dateWidth(segment.DateFormat) > dateWidth(best) {
			best = segment.DateFormat
		}
	}
	return best
}

func (template PathTemplate) Match(path string) (PathTemplateMatch, bool) {
	result := PathTemplateMatch{Slots: []PathDateSlot{}}
	for _, segment := range template.Segments {
		width := dateWidth(segment.DateFormat)
		if width == 0 {
			if !strings.HasPrefix(path, segment.Literal) {
				return PathTemplateMatch{}, false
			}
			path = path[len(segment.Literal):]
			continue
		}
		if len(path) < width {
			return PathTemplateMatch{}, false
		}
		value := path[:width]
		for _, char := range value {
			if char != '-' && (char < '0' || char > '9') {
				return PathTemplateMatch{}, false
			}
		}
		result.Slots = append(result.Slots, PathDateSlot{Format: segment.DateFormat, Value: value})
		if len(value) >= len(result.MostSpecificValue) {
			result.MostSpecificValue = value
		}
		path = path[width:]
	}
	return result, path == ""
}
