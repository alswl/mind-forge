package content

import (
	"testing"
)

func TestIndexedBuildSelectorPrecedenceAndAmbiguity(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "mind-index.yaml", `schema: '1'
articles:
- title: display-only
  article_path: docs/post/
- title: other
  article_path: custom/post.md
  template_origin: {template_name: daily, slot_value: current}
- title: duplicate
  article_path: first/shared.md
- title: duplicate
  article_path: second/shared.md
`)
	for selector, want := range map[string]string{"post": "docs/post/", "docs/post": "docs/post/", "docs/post/": "docs/post/", "daily/current": "custom/post.md", "custom/post.md": "custom/post.md"} {
		item, err := ResolveIndexedArticle(root, selector)
		if err != nil || item.Path != want {
			t.Fatalf("%q: %+v %v", selector, item, err)
		}
	}
	for _, selector := range []string{"shared", "display-only", "missing"} {
		if _, err := ResolveIndexedArticle(root, selector); err == nil {
			t.Fatalf("accepted %q", selector)
		}
	}
}
