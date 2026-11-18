package term

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

type SelectionRequest struct {
	Selection    Selection
	Terms, AdHoc []string
	AdHocFrom    *string
}

func BuildSelection(cwd string, request SelectionRequest) (Selection, error) {
	s := request.Selection

	for _, raw := range request.Terms {
		name, original, pair := strings.Cut(raw, ":")
		if name == "" || pair && original == "" {
			return s, output.Usage("invalid qualified term selector '"+raw+"'", "use --term NAME or --term NAME:ORIGINAL")
		}
		if pair {
			s.Pairs = append(s.Pairs, Pair{Term: name, Original: original})
		} else {
			s.Terms = append(s.Terms, name)
		}
	}
	rules := request.AdHoc
	if path := request.AdHocFrom; path != nil {
		name := *path
		if !filepath.IsAbs(name) {
			name = filepath.Join(cwd, name)
		}
		raw, err := os.ReadFile(name)
		if err != nil {
			return s, err
		}
		for _, line := range strings.Split(string(raw), "\n") {
			line = strings.TrimSpace(line)
			if line != "" && !strings.HasPrefix(line, "#") {
				rules = append(rules, line)
			}
		}
	}
	for _, raw := range rules {
		original, correct, ok := strings.Cut(raw, "=>")
		if !ok {
			return s, output.Usage("invalid ad-hoc correction '"+raw+"': expected ORIGINAL=>CORRECT", "")
		}
		original, correct = strings.TrimSpace(original), strings.TrimSpace(correct)
		if original == "" || correct == "" {
			return s, output.Usage("invalid ad-hoc correction '"+raw+"': both sides must be non-empty", "")
		}
		s.AdHoc = append(s.AdHoc, model.Correction{Original: original, Correct: correct})
	}
	return s, nil
}
