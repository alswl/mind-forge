package lifecycle

import (
	"errors"
	"strings"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestAuthoredReferencesPreserveBytesOutsideDestinations(t *testing.T) {
	target := "p/brief/assets"
	moves := map[string]*string{"p/assets/a": &target}
	text := "[one](../assets/a/x.png \"title\")\r\n![two](<../assets/a/x.png>)\r\n[three][id]\r\n[id]: ../assets/a/x.png#part\r\n<img src='../assets/a/x.png'><a href=../assets/a/x.png>link</a>\r\n`[code](../assets/a/x.png)`\r\n```md\r\n![code](../assets/a/x.png)\r\n```"
	got, err := RewriteAuthoredReferences(text, "p/docs/a.md", "p/docs/a.md", moves)
	if err != nil {
		t.Fatal(err)
	}
	want := strings.Replace(text, "../assets/a/x.png", "../brief/assets/x.png", 5)
	if got != want {
		t.Fatalf("got %q\nwant %q", got, want)
	}
}

func TestAuthoredReferencesRebaseAndRejectUnverifiedSyntax(t *testing.T) {
	target := "q/x/a.md"
	moves := map[string]*string{"p/a.md": &target}
	for _, tc := range []struct{ text, want, kind string }{
		{"[x](assets/a%20b.png?q=1#p)", "[x](../../p/assets/a%20b.png?q=1#p)", ""},
		{"[x](assets/a\\(b\\).png)", "[x](../../p/assets/a%28b%29.png)", ""},
		{"[x](https://example.test/a) ![x](data:image/png;base64,a) [x](#part)", "[x](https://example.test/a) ![x](data:image/png;base64,a) [x](#part)", ""},
		{"[[a.md]]", "", "reference_unsupported"},
		{"[x]({{assets}}/a/file)", "", "reference_unsupported"},
		{"[x](/assets/a/file)", "", "reference_unsupported"},
		{"[x](assets/%FF)", "", "reference_unsupported"},
		{"[x](assets/%2)", "", "reference_unsupported"},
		{"[x](a.md \"unterminated)", "", "reference_unsupported"},
		{"[x](a.md junk)", "", "reference_unsupported"},
		{"<img srcset='a.md 1x'>", "", "reference_unsupported"},
		{"<img style='background:a.md'>", "", "reference_unsupported"},
		{"<img xlink:href='a.md'>", "", "reference_unsupported"},
		{"<img src='a.md'", "", "reference_unsupported"},
		{"> [id]: a.md", "", "reference_unsupported"},
	} {
		t.Run(tc.text, func(t *testing.T) {
			got, err := RewriteAuthoredReferences(tc.text, "p/a.md", target, moves)
			if tc.kind == "" {
				if err != nil || got != tc.want {
					t.Fatalf("got %q err %v want %q", got, err, tc.want)
				}
			} else {
				var failure *output.Error
				if !errors.As(err, &failure) || failure.Kind != tc.kind {
					t.Fatalf("got %q err %v want %s", got, err, tc.kind)
				}
			}
		})
	}
}

func TestAuthoredReferencesMaskCodeAndPreserveTitle(t *testing.T) {
	target := "p/b.md"
	moves := map[string]*string{"p/a.md": &target}
	text := "> ```md\n> [x](a.md)\n> ```\n<code>[x](a.md)</code>\n\\[escaped](a.md)\n    [indented](a.md)\n<!-- [x](a.md) -->\n<script>[x](a.md)</script>\n``[code ` x](a.md)``"
	got, err := RewriteAuthoredReferences(text, "p/index.md", "p/index.md", moves)
	if err != nil || got != text {
		t.Fatalf("masked content changed: %q %v", got, err)
	}
	text = "[x](a.md \"title [x](a.md)\")"
	got, err = RewriteAuthoredReferences(text, "p/index.md", "p/index.md", moves)
	if err != nil || got != "[x](b.md \"title [x](a.md)\")" {
		t.Fatalf("title changed: %q %v", got, err)
	}
}

func TestAuthoredReferencesRefuseInboundDeletionAndChooseLongestMove(t *testing.T) {
	target := "p/new"
	moves := map[string]*string{"p/assets": &target, "p/assets/a": nil}
	_, err := RewriteAuthoredReferences("[x](assets/a/file)", "p/a.md", "p/a.md", moves)
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "inbound_reference" {
		t.Fatal(err)
	}
	got, err := RewriteAuthoredReferences("[x](assets/ab/file)", "p/a.md", "p/a.md", moves)
	if err != nil || got != "[x](new/ab/file)" {
		t.Fatalf("%q %v", got, err)
	}
}
