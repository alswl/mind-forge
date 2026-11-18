package store

import "os"

func ExportFile(path string, raw []byte) error {
	return os.WriteFile(path, raw, 0666)
}
