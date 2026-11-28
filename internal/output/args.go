package output

// WantsJSON detects an explicit format when parsing fails before flags are bound.
func WantsJSON(args []string) bool {
	previousOutput := false
	for _, arg := range args {
		if arg == "--json" || previousOutput && arg == "json" || arg == "--output=json" || arg == "-o=json" || arg == "-ojson" {
			return true
		}
		previousOutput = arg == "--output" || arg == "-o"
	}
	return false
}
