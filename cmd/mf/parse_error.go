package main

import (
	"errors"
	"fmt"
	"io"
	"os"
	"slices"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

type parseFailure struct {
	kind, value, label string
	detail             string
	flag               FlagSpec
	missing            []string
	other              string
}

func (failure *parseFailure) Error() string {
	return failure.kind + ": " + failure.value
}

func flagLabel(flag FlagSpec) string {
	label := "--" + flag.Name
	if flag.ValueName != "" {
		label += " <" + flag.ValueName + ">"
	} else if flag.Kind == "count" {
		label += "..."
	}
	return label
}

func flagParseError(command *cobra.Command, err error) error {
	// pflag continues past positional tokens. A command group must reject an
	// unknown subcommand before reporting an error in a later option.
	if args := command.Flags().Args(); command.HasSubCommands() && len(args) > 0 {
		spec, loadError := LoadSpec()
		if loadError != nil {
			return loadError
		}
		item, ok := spec.Lookup(strings.Fields(command.CommandPath())[1:])
		if ok && len(item.Arguments) == 0 {
			return &parseFailure{kind: "unknown_command", value: args[0]}
		}
	}
	var failure *parseFailure
	if errors.As(err, &failure) {
		return failure
	}
	var unknown *pflag.NotExistError
	if errors.As(err, &unknown) {
		value := "--" + unknown.GetSpecifiedName()
		if unknown.GetSpecifiedShortnames() != "" {
			value = "-" + unknown.GetSpecifiedName()
		}
		return &parseFailure{kind: "unexpected_flag", value: value}
	}
	var required *pflag.ValueRequiredError
	if errors.As(err, &required) {
		if value, ok := required.GetFlag().Value.(*checkedValue); ok {
			return &parseFailure{kind: "value_required", flag: value.spec}
		}
	}
	return err
}

// ParseErrorText preserves the public parser's text independently of Cobra's
// diagnostic wording. The same text becomes the JSON error message.
func ParseErrorText(spec *Spec, item CommandSpec, failure *parseFailure, args []string) string {
	// A short boolean followed by '=' is parsed as a short-option cluster:
	// the boolean is accepted and the '=' becomes the unexpected option.
	if failure.kind == "unexpected_value" && failure.flag.Short != "" && (failure.flag.Kind == "bool" || failure.flag.Kind == "count") {
		for index, arg := range args {
			if strings.HasPrefix(arg, "-"+failure.flag.Short+"=") {
				copy := *failure
				copy.kind, copy.value = "unexpected_flag", "-="
				copy.flag = FlagSpec{}
				failure = &copy
				args = append(append([]string{}, args[:index]...), args[index+1:]...)
				break
			}
		}
	}
	if failure.kind == "subcommand_required" {
		if failure.label != "" {
			return failure.label
		}
		if item.HelpDefault != "" {
			return item.HelpDefault
		}
		return unwrappedHelp(item.Help)
	}
	usage := "mf " + item.Usage
	if item.Usage == "" {
		usage = "mf"
	}
	var body strings.Builder
	flag := failure.flag
	label := failure.label
	if label == "" && flag.Name != "" {
		label = flagLabel(flag)
	}
	choices := flag.Choices
	switch failure.kind {
	case "invalid_value", "value_required":
		if failure.kind == "invalid_value" {
			fmt.Fprintf(&body, "error: invalid value '%s' for '%s'", failure.value, label)
			if failure.detail != "" {
				fmt.Fprintf(&body, ": %s", failure.detail)
			}
			body.WriteByte('\n')
		} else {
			fmt.Fprintf(&body, "error: a value is required for '%s' but none was supplied\n", label)
		}
		if len(choices) > 0 {
			fmt.Fprintf(&body, "  [possible values: %s]\n", strings.Join(choices, ", "))
		}
		if suggestion := mostSimilar(failure.value, choices); failure.kind == "invalid_value" && suggestion != "" {
			fmt.Fprintf(&body, "\n  tip: a similar value exists: '%s'\n", suggestion)
		}
		usage = ""
	case "duplicate":
		fmt.Fprintf(&body, "error: the argument '%s' cannot be used multiple times\n", label)
	case "unexpected_value":
		fmt.Fprintf(&body, "error: unexpected value '%s' for '%s' found; no more were expected\n", failure.value, label)
		usage = contextualUsage(spec, item, args, []string{label}, nil)
	case "conflict":
		fmt.Fprintf(&body, "error: the argument '%s' cannot be used with '%s'\n", label, failure.other)
		usage = contextualUsage(spec, item, nil, []string{label}, nil)
	case "missing":
		body.WriteString("error: the following required arguments were not provided:\n")
		for _, label := range failure.missing {
			fmt.Fprintf(&body, "  %s\n", label)
		}
		usage = contextualUsage(spec, item, args, failure.missing, nil)
	case "unknown_command":
		fmt.Fprintf(&body, "error: unrecognized subcommand '%s'\n", failure.value)
		var names []string
		for _, child := range spec.Commands {
			if len(child.Path) == len(item.Path)+1 && slices.Equal(child.Path[:len(item.Path)], item.Path) {
				names = append(names, child.Path[len(item.Path)])
				names = append(names, child.Aliases...)
			}
		}
		if suggestion := mostSimilar(failure.value, names); suggestion != "" {
			fmt.Fprintf(&body, "\n  tip: a similar subcommand exists: '%s'\n", suggestion)
		}
	case "unexpected_flag", "unexpected_argument":
		fmt.Fprintf(&body, "error: unexpected argument '%s' found\n", failure.value)
		if failure.kind == "unexpected_flag" {
			var names []string
			for _, flag := range append(append([]FlagSpec{}, item.Flags...), spec.GlobalFlags...) {
				if !flag.Hidden {
					names = append(names, flag.Name)
					names = append(names, flag.Aliases...)
				}
			}
			var suggested []string
			if strings.HasPrefix(failure.value, "--") {
				if suggestion := mostSimilar(strings.TrimPrefix(failure.value, "--"), names); suggestion != "" {
					fmt.Fprintf(&body, "\n  tip: a similar argument exists: '--%s'\n", suggestion)
					if flag, ok := spec.Flag(item.Path, "--"+suggestion); ok {
						suggested = append(suggested, flagLabel(flag))
					}
				}
			}
			if len(suggested) == 0 && positionalAvailable(spec, item, args) {
				fmt.Fprintf(&body, "\n  tip: to pass '%s' as a value, use '-- %s'\n", failure.value, failure.value)
			}
			usage = contextualUsage(spec, item, args, suggested, nil)
		}
	default:
		fmt.Fprintf(&body, "error: %s\n", failure.value)
	}
	if usage != "" {
		fmt.Fprintf(&body, "\nUsage: %s\n", usage)
	}
	body.WriteString("\nFor more information, try '--help'.\n")
	return body.String()
}

func positionalAvailable(spec *Spec, item CommandSpec, args []string) bool {
	var current []string
	count := 0
	for index := 0; index < len(args); index++ {
		token := args[index]
		if token == "--" {
			count += len(args) - index - 1
			break
		}
		name := strings.SplitN(token, "=", 2)[0]
		if flag, ok := spec.Flag(current, name); ok {
			if flag.Kind != "bool" && flag.Kind != "count" && name == token {
				index++
			}
			continue
		}
		if strings.HasPrefix(token, "-") {
			break
		}
		candidate := append(append([]string{}, current...), token)
		if command, ok := spec.Lookup(candidate); ok {
			current = command.Path
			count = 0
		} else {
			count++
		}
	}
	return count < len(item.Arguments) || len(item.Arguments) > 0 && item.Arguments[len(item.Arguments)-1].Repeat
}

func contextualUsage(spec *Spec, item CommandSpec, args, added, excluded []string) string {
	// Clap excludes global flags supplied before the selected subcommand from
	// that subcommand's contextual usage. Values are consumed before matching
	// command names, so a path named "config" cannot change the context.
	var current []string
	var used []string
	for position := 0; position < len(args); position++ {
		token := args[position]
		if token == "--" {
			break
		}
		name := strings.SplitN(token, "=", 2)[0]
		if flag, ok := spec.Flag(current, name); ok {
			used = append(used, flagLabel(flag))
			if flag.Kind != "bool" && flag.Kind != "count" && name == token {
				position++
			}
			continue
		}
		candidate := append(append([]string{}, current...), token)
		if command, ok := spec.Lookup(candidate); ok {
			current = command.Path
			used = nil
		}
	}
	labels := []string{}
	for _, label := range append(append([]string{}, added...), used...) {
		if !slices.Contains(labels, label) && !slices.Contains(excluded, label) {
			labels = append(labels, label)
		}
	}
	if len(labels) == 0 {
		return strings.TrimSpace("mf " + item.Usage)
	}
	var requiredFlags []string
	for _, flag := range item.Flags {
		if flag.Required {
			requiredFlags = append(requiredFlags, flagLabel(flag))
		}
	}
	labels = slices.DeleteFunc(labels, func(label string) bool { return slices.Contains(requiredFlags, label) })
	labels = append(requiredFlags, labels...)
	// Required positional arguments follow flags even when already supplied.
	var positional []string
	labels = slices.DeleteFunc(labels, func(label string) bool {
		if strings.HasPrefix(label, "<") {
			return true
		}
		return false
	})
	for _, argument := range item.Arguments {
		label := "<" + argument.Name + ">"
		if argument.Required {
			if argument.Repeat {
				label += "..."
			}
		} else {
			label = "[" + argument.Name + "]"
			if argument.Repeat {
				label += "..."
			}
		}
		positional = append(positional, label)
	}
	labels = append(labels, positional...)
	if item.RequiredSubcommand {
		labels = append(labels, "<COMMAND>")
	}
	if len(labels) == 0 {
		return "mf " + item.Usage
	}
	return strings.TrimSpace("mf "+strings.Join(item.Path, " ")) + " " + strings.Join(labels, " ")
}

func unwrappedHelp(help string) string {
	lines := strings.Split(help, "\n")
	var result []string
	for _, line := range lines {
		if strings.HasPrefix(line, strings.Repeat(" ", 20)) && len(result) > 0 {
			result[len(result)-1] += " " + strings.TrimSpace(line)
		} else {
			result = append(result, line)
		}
	}
	return strings.Join(result, "\n")
}

func mostSimilar(value string, names []string) string {
	type candidate struct {
		name  string
		score float64
	}
	var candidates []candidate
	for _, name := range names {
		if score := jaro(value, name); score > 0.7 {
			candidates = append(candidates, candidate{name, score})
		}
	}
	sort.SliceStable(candidates, func(i, j int) bool { return candidates[i].score < candidates[j].score })
	if len(candidates) == 0 {
		return ""
	}
	return candidates[len(candidates)-1].name
}

func jaro(left, right string) float64 {
	a, b := []rune(left), []rune(right)
	if len(a) == 0 && len(b) == 0 {
		return 1
	}
	if len(a) == 0 || len(b) == 0 {
		return 0
	}
	distance := max(len(a), len(b))/2 - 1
	if distance < 0 {
		distance = 0
	}
	usedA, usedB := make([]bool, len(a)), make([]bool, len(b))
	matches := 0
	for i, char := range a {
		for j := max(0, i-distance); j < min(len(b), i+distance+1); j++ {
			if !usedB[j] && char == b[j] {
				usedA[i], usedB[j] = true, true
				matches++
				break
			}
		}
	}
	if matches == 0 {
		return 0
	}
	transpositions, index := 0, 0
	for i, char := range a {
		if !usedA[i] {
			continue
		}
		for !usedB[index] {
			index++
		}
		if char != b[index] {
			transpositions++
		}
		index++
	}
	m := float64(matches)
	return (m/float64(len(a)) + m/float64(len(b)) + (m-float64(transpositions)/2)/m) / 3
}

func renderParseFailure(command *cobra.Command, spec *Spec, failure *parseFailure, args []string, stderr io.Writer) int {
	jsonError := wantsJSON(args)
	if settings, ok := runtimeFromCommand(command); ok && settings.Output == "json" {
		jsonError = true
	}
	path := strings.Fields(command.CommandPath())[1:]
	item, ok := spec.Lookup(path)
	if !ok {
		item, _ = spec.Lookup(nil)
	}
	if failure.kind == "subcommand_required" && os.Getenv("COLUMNS") == "80" {
		failure.label = item.Help
	}
	text := ParseErrorText(spec, item, failure, args)
	if (failure.kind == "invalid_value" || failure.kind == "value_required") && failure.flag.Name == "output" && slices.Contains(args, "build") {
		format := "text"
		if jsonError {
			format = "json"
		}
		err := output.Usage(strings.TrimSpace(strings.TrimPrefix(text, "error:")), "to set the build output path, use --out instead")
		if output.RenderError(stderr, format, err) != nil {
			return 1
		}
		return 2
	}
	if jsonError {
		err := output.Usage(strings.TrimSpace(text), "")
		if output.RenderError(stderr, "json", err) != nil {
			return 1
		}
	} else if _, err := stderr.Write([]byte(text)); err != nil {
		return 1
	}
	return 2
}
