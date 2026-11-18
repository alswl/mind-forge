package output

import "strings"

type List struct {
	Headers []string
	Rows    [][]ListCell
	Plural  string
}

type ListOptions struct {
	NoHeaders    bool
	NoTrunc      bool
	Width        int
	ColorEnabled bool
	DisplayOptions
}

type ListCell struct {
	Text                    string
	Optional, Present, Path bool
	ANSIPrefix, ANSISuffix  string
}

func TextCells(values ...string) []ListCell {
	cells := make([]ListCell, 0, len(values))
	for _, value := range values {
		cells = append(cells, ListCell{Text: value})
	}
	return cells
}

func (cell ListCell) raw() string {
	if cell.Optional && !cell.Present {
		return "-"
	}
	return cell.Text
}

func (cell ListCell) render(options ListOptions) string {
	if cell.Path {
		return RenderPathLink(cell.Text, options.DisplayOptions)
	}
	text := cell.raw()
	if options.ColorEnabled && cell.ANSIPrefix != "" {
		suffix := cell.ANSISuffix
		if suffix == "" {
			suffix = "\x1b[0m"
		}
		return cell.ANSIPrefix + text + suffix
	}
	return text
}

func ListOptionsFromProfile(profile TerminalProfile, noHeaders, noTrunc bool) ListOptions {
	policy := RenderingPolicy(profile, "text")
	options := ListOptions{NoHeaders: noHeaders || !profile.StdoutIsTTY, NoTrunc: noTrunc || !profile.StdoutIsTTY, ColorEnabled: policy.EmitANSIColor, DisplayOptions: DisplayOptions{Hyperlinks: policy.EmitHyperlinks}}
	if profile.StdoutIsTTY {
		options.Width = profile.TerminalWidth
	}
	return options
}

func RenderList(view List, options ListOptions) string {
	if len(view.Rows) == 0 {
		return "No " + view.Plural + " found.\n"
	}
	widths := make([]int, len(view.Headers))
	for index, header := range view.Headers {
		widths[index] = DisplayWidth(header)
	}
	for _, row := range view.Rows {
		for index, cell := range row {
			if index < len(widths) {
				widths[index] = max(widths[index], DisplayWidth(cell.raw()))
			}
		}
	}
	total := max(0, len(widths)-1) * 2
	for _, width := range widths {
		total += width
	}
	truncate := !options.NoTrunc && options.Width > 0 && total > options.Width
	var output strings.Builder
	writeRow := func(cells []ListCell) {
		used := 0
		for index, cell := range cells {
			if index >= len(widths) {
				break
			}
			if index > 0 {
				output.WriteString("  ")
				used += 2
			}
			width := DisplayWidth(cell.raw())
			text := cell.render(options)
			if index == 0 {
				output.WriteString(text)
				used += width
				if index < len(cells)-1 {
					output.WriteString(strings.Repeat(" ", max(0, widths[index]-width)))
				}
				used = max(used, widths[index])
				continue
			}
			if truncate && max(0, options.Width-used) < min(widths[index], 3) {
				break
			}
			if truncate && used+widths[index] > options.Width {
				available := max(0, options.Width-used)
				if available > 1 {
					output.WriteString(truncateCell(text, width, available))
				}
				break
			}
			output.WriteString(text)
			if index < len(cells)-1 {
				output.WriteString(strings.Repeat(" ", max(0, widths[index]-width)))
			}
			used += widths[index]
		}
		output.WriteByte('\n')
	}
	if !options.NoHeaders {
		writeRow(TextCells(view.Headers...))
	}
	for _, row := range view.Rows {
		writeRow(row)
	}
	return output.String()
}

func truncateCell(text string, width, available int) string {
	if width <= available {
		return text
	}
	if available <= 1 {
		return "…"
	}
	var prefix strings.Builder
	used := 0
	for _, char := range text {
		width := DisplayWidth(string(char))
		if used+width > available-1 {
			break
		}
		prefix.WriteRune(char)
		used += width
	}
	return prefix.String() + "…"
}

func JSONCollection[T any](plural string, items []T) map[string]any {
	if items == nil {
		items = make([]T, 0)
	}
	return map[string]any{plural: items}
}
