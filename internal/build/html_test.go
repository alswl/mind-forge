package build

import (
	"crypto/sha256"
	"encoding/base64"
	"strings"
	"testing"
)

func TestHTMLTemplateStandalonePlaceholder(t *testing.T) {
	for _, input := range []string{"", "<main>missing</main>", "<main>{{content}}{{content}}</main>", "<main>prefix {{content}}</main>", "<main data-x='{{content}}'></main>", "<script>{{content}}</script>"} {
		if _, err := PrepareHTMLTemplate(input); err == nil {
			t.Errorf("accepted invalid template %q", input)
		}
	}
	value, err := PrepareHTMLTemplate("<main onclick='alert(1)'>{{content}}</main><script>alert(2)</script><iframe src='https://example.invalid'></iframe>")
	if err != nil {
		t.Fatal(err)
	}
	for _, bad := range []string{"onclick", "<script", "<iframe"} {
		if strings.Contains(value, bad) {
			t.Errorf("unsafe template %q", value)
		}
	}
	if !strings.Contains(value, "{{content}}") {
		t.Fatal("lost valid placeholder")
	}
}

func TestHTMLMarkdownAndRawHTML(t *testing.T) {
	template, err := PrepareHTMLTemplate("<main class='mf-content'>{{content}}</main>")
	if err != nil {
		t.Fatal(err)
	}
	input := "---\ntitle: Hidden metadata\n---\n# Public\n\n~~old~~\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```go\nx < y\n```\n\n<a onclick=\"alert(1)\">literal</a>\n\n[bad](javascript:alert%281%29)\n\n![diagram](assets/a.png)\n"
	got, err := RenderHTML(input, "<&\"title", template)
	if err != nil {
		t.Fatal(err)
	}
	for _, part := range []string{"<!doctype html>", "<h1>Public</h1>", "<table>", "<del>old</del>", "language-go", "x &lt; y", "&lt;a onclick=", "assets/a.png", "<title>&lt;&amp;&#34;title</title>"} {
		if !strings.Contains(got, part) {
			t.Errorf("missing %q", part)
		}
	}
	if strings.Contains(got, "Hidden metadata") || strings.Contains(got, "href=\"javascript:") || strings.Contains(got, "<a onclick=") {
		t.Fatalf("unsafe or unfiltered HTML: %s", got)
	}
}

func TestHTMLMermaidSourceAndCSP(t *testing.T) {
	template, err := PrepareHTMLTemplate(DefaultHTMLTemplate)
	if err != nil {
		t.Fatal(err)
	}
	got, err := RenderHTML("```mermaid\ngraph TD\n A[\"<script>x</script>\"] --> B\n```\n", "Public", template)
	if err != nil {
		t.Fatal(err)
	}
	hash := sha256.Sum256([]byte(htmlRuntime))
	for _, part := range []string{"mf-diagram", "aria-live=\"polite\"", "Diagram source", "&lt;script&gt;x&lt;/script&gt;", "Mermaid diagram is not rendered yet", "script-src 'sha256-" + base64.StdEncoding.EncodeToString(hash[:]), "connect-src 'none'", "object-src 'none'", "15000", "securityLevel: 'strict'", "parse-error", "load-error"} {
		if !strings.Contains(got, part) {
			t.Errorf("missing %q", part)
		}
	}
}

func TestHTMLTitleCannotIntroduceURLOrPlaceholder(t *testing.T) {
	got, err := RenderHTML("# Public", "javascript:alert(1)", "<a href=\"{{title}}\">Link</a><main>{{content}}</main>")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(got, "href=\"javascript:") {
		t.Fatal("title introduced active URL")
	}
	if !strings.Contains(got, "<h1>Public</h1>") {
		t.Fatal("title substitution removed public content")
	}
	got, err = RenderHTML("Literal {{title}} and {{content}}.", "Title", "<header>{{title}}</header><main>{{content}}</main>")
	if err != nil || !strings.Contains(got, "Literal {{title}} and {{content}}.") {
		t.Fatalf("body placeholders substituted: %s %v", got, err)
	}
}
