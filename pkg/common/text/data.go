package text

import (
	"bytes"
	"compress/gzip"
	_ "embed"
	"io"
)

//go:embed data/dict.txt.gz
var dictionaryData []byte

//go:embed data/hmm.model.gz
var hmmData []byte

//go:embed data/pinyin-first.tsv.gz
var pinyinData []byte

func unpack(data []byte) []byte {
	r, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		panic(err)
	}
	defer r.Close()
	b, err := io.ReadAll(r)
	if err != nil {
		panic(err)
	}
	return b
}
