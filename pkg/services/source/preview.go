package source

type NewPreview struct{ Name, Input string }

func PreviewNew(input string, name *string) NewPreview {
	value, err := DeriveNameFromPath(input)
	if err != nil {
		value = "unknown"
	}
	if name != nil {
		value = *name
	}
	return NewPreview{Name: value, Input: input}
}
