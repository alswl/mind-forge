package cli

func hasPathSelector(path []string) bool {
	if len(path) == 0 {
		return false
	}
	switch path[0] {
	case "build", "render":
		return len(path) == 1
	case "publish":
		return len(path) == 2 && (path[1] == "run" || path[1] == "update")
	case "prompt", "thinking":
		return len(path) == 2 && path[1] == "show"
	case "article":
		if len(path) == 3 && path[1] == "block" {
			return true
		}
		if len(path) != 2 {
			return false
		}
		switch path[1] {
		case "show", "update", "remove", "move", "rename":
			return true
		}
	case "asset":
		if len(path) != 2 {
			return false
		}
		switch path[1] {
		case "show", "remove", "rename", "move":
			return true
		}
	case "source":
		if len(path) != 2 {
			return false
		}
		switch path[1] {
		case "show", "update", "remove", "rename", "move":
			return true
		}
	}
	return false
}
