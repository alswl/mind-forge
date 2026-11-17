package content

import (
	"reflect"
	"testing"
)

func TestArticleTemplateBlocksIgnoreFencesAndPreserveOpening(t *testing.T) {
	blocks, err := SplitArticleTemplate("# Title\r\n\r\n```md\r\n## Literal\r\n```\r\n\r\n## 本周进展\r\nbody\r\n")
	if err != nil {
		t.Fatal(err)
	}
	want := []ArticleBlock{{"01-opening.md", "# Title\n\n```md\n## Literal\n```\n"}, {"02-本周进展.md", "## 本周进展\nbody"}}
	if !reflect.DeepEqual(blocks, want) {
		t.Fatalf("blocks %#v want %#v", blocks, want)
	}
	if _, err := SplitArticleTemplate("# T\n## Same Name\n## Same_Name\n"); err == nil {
		t.Fatal("duplicate heading slug accepted")
	}
	blank, err := SplitArticleTemplate("")
	if err != nil || len(blank) != 1 || blank[0].Body != "" {
		t.Fatalf("empty template %#v %v", blank, err)
	}
}

func TestArticleBuiltinTemplates(t *testing.T) {
	for name, count := range map[string]int{"blank": 1, "blog": 3, "arch": 5, "prd": 5} {
		body, kind, ok := BuiltinArticleTemplate(name)
		if !ok || kind != name {
			t.Fatalf("%s template unavailable", name)
		}
		blocks, err := SplitArticleTemplate(body)
		if err != nil || len(blocks) != count {
			t.Fatalf("%s blocks %#v %v", name, blocks, err)
		}
	}
	if _, _, ok := BuiltinArticleTemplate("BLOG"); ok {
		t.Fatal("case alias added")
	}
}
