package textcase

func ASCIILower(text string) string {
	result := []byte(text)
	for i, b := range result {
		if b >= 'A' && b <= 'Z' {
			result[i] = b + 'a' - 'A'
		}
	}
	return string(result)
}

func ASCIIEqualFold(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := 0; index < len(left); index++ {
		a, b := left[index], right[index]
		if a >= 'A' && a <= 'Z' {
			a += 'a' - 'A'
		}
		if b >= 'A' && b <= 'Z' {
			b += 'a' - 'A'
		}
		if a != b {
			return false
		}
	}
	return true
}
