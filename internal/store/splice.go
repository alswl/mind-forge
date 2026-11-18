package store

import "bytes"

func topLevelSpan(raw []byte, key string) (int, int, bool) {
	bare := []byte(key + ":")
	prefix := []byte(key + ": ")
	start, trailer := -1, -1
	for offset := 0; offset < len(raw); {
		length := bytes.IndexByte(raw[offset:], '\n') + 1
		if length == 0 {
			length = len(raw) - offset
		}
		line := bytes.TrimRight(raw[offset:offset+length], "\r\n")
		if start < 0 {
			if bytes.Equal(line, bare) || bytes.HasPrefix(line, prefix) {
				start = offset
			}
		} else if len(line) == 0 || line[0] == '#' {
			if trailer < 0 {
				trailer = offset
			}
		} else if line[0] == ' ' || line[0] == '\t' || line[0] == '-' {
			trailer = -1
		} else {
			if trailer >= 0 {
				return start, trailer, true
			}
			return start, offset, true
		}
		offset += length
	}
	if trailer >= 0 {
		return start, trailer, start >= 0
	}
	return start, len(raw), start >= 0
}
