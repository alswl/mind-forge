package content

import "github.com/alswl/mind-forge/pkg/common/model"

func ListOutputsRequest(root, cwd string, selected, selector *string) (model.JSONArray[model.Output], error) {
	project, err := ProjectFor(root, cwd, selected, selector)
	if err != nil {
		return nil, err
	}
	var minds []model.Mind
	if selector != nil {
		mind, loadErr := ResolveMind(root, project, *selector)
		minds = []model.Mind{mind}
		err = loadErr
	} else {
		minds, err = Discover(root, project)
	}
	outputs := model.JSONArray[model.Output]{}
	for _, mind := range minds {
		outputs = append(outputs, mind.Outputs...)
	}
	if err != nil {
		return nil, err
	}
	return outputs, nil
}
