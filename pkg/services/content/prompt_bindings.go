package content

import "github.com/alswl/mind-forge/pkg/common/model"

type promptBinding struct {
	Prompt model.Prompt
	Status string
}

func resolvePromptBindings(index model.Index) []promptBinding {
	existing := map[string]bool{}
	if index.Articles != nil {
		for _, article := range *index.Articles {
			existing[article.Path] = true
		}
	}
	bindings := []promptBinding{}
	if index.Prompts == nil {
		return bindings
	}
	counts := map[string]int{}
	for _, prompt := range *index.Prompts {
		if prompt.Article != "" && existing[prompt.Article] {
			counts[prompt.Article]++
		}
	}
	for _, prompt := range *index.Prompts {
		status := "orphan"
		if prompt.Article != "" && existing[prompt.Article] {
			status = "bound"
			if counts[prompt.Article] > 1 {
				status = "duplicate"
			}
		}
		bindings = append(bindings, promptBinding{Prompt: prompt, Status: status})
	}
	return bindings
}
