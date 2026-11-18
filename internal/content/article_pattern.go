package content

func articlePattern(pattern, name string) ([]PathSegment, error) {
	template, err := ParsePathTemplate(pattern)
	if err != nil {
		return nil, err
	}
	if err := template.ValidateSlots(name); err != nil {
		return nil, err
	}
	return template.Segments, nil
}

func matchArticlePattern(segments []PathSegment, path string) (string, bool) {
	result, ok := (PathTemplate{Segments: segments}).Match(path)
	return result.MostSpecificValue, ok
}
