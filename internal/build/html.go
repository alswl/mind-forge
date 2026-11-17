package build

import (
	"bytes"
	"crypto/sha256"
	_ "embed"
	"encoding/base64"
	"fmt"
	"html"
	"strings"

	"github.com/alswl/mind-forge/internal/markdown"
	"github.com/microcosm-cc/bluemonday"
	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/ast"
	"github.com/yuin/goldmark/extension"
	"github.com/yuin/goldmark/renderer"
	"github.com/yuin/goldmark/util"
	nethtml "golang.org/x/net/html"
)

//go:embed assets/html.js
var htmlRuntime string

//go:embed assets/html.css
var htmlStyle string

const DefaultHTMLTemplate = `<main class="mf-content">{{content}}</main>`

func cleanHTML(value string) string {
	p := bluemonday.UGCPolicy()
	p.AllowElements("main", "figure", "figcaption", "details", "summary")
	p.AllowAttrs("class", "aria-live").Globally()
	p.AllowAttrs("open").OnElements("details")
	p.RequireNoFollowOnLinks(false)
	p.RequireNoReferrerOnLinks(true)
	return p.Sanitize(value)
}

func PrepareHTMLTemplate(value string) (string, error) {
	value = cleanHTML(value)
	doc, err := nethtml.Parse(strings.NewReader(value))
	if err != nil {
		return "", err
	}
	standalone := 0
	var visit func(*nethtml.Node)
	visit = func(n *nethtml.Node) {
		if n.Type == nethtml.TextNode && strings.TrimSpace(n.Data) == "{{content}}" {
			standalone++
		}
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			visit(c)
		}
	}
	visit(doc)
	if strings.Count(value, "{{content}}") != 1 || standalone != 1 {
		return "", fmt.Errorf("HTML body template must contain exactly one standalone {{content}} text placeholder after safety filtering")
	}
	return value, nil
}

// RenderHTML consumes assembled public Markdown. The renderer alone owns the
// executable runtime and head; source HTML is displayed as escaped text.
func RenderHTML(source, title, template string) (string, error) {
	_, body, eol, ok := markdown.InitialFrontMatter(source)
	if ok {
		source = strings.TrimPrefix(body, eol)
	}
	md := goldmark.New(goldmark.WithExtensions(extension.Table, extension.Strikethrough), goldmark.WithRendererOptions(renderer.WithNodeRenderers(util.Prioritized(safeHTMLRenderer{}, 100))))
	var output bytes.Buffer
	if err := md.Convert([]byte(source), &output); err != nil {
		return "", err
	}
	escapedTitle := html.EscapeString(title)
	before, after, ok := strings.Cut(template, "{{content}}")
	if !ok {
		return "", fmt.Errorf("HTML body template is missing {{content}}")
	}
	before = strings.ReplaceAll(before, "{{title}}", escapedTitle)
	after = strings.ReplaceAll(after, "{{title}}", escapedTitle)
	bodyHTML := cleanHTML(before + output.String() + after)
	hash := sha256.Sum256([]byte(htmlRuntime))
	return fmt.Sprintf("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'sha256-%s' https://cdn.jsdelivr.net/npm/mermaid@11.12.0/; style-src 'unsafe-inline'; img-src 'self' https: http:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>%s</title><style>%s</style></head><body>%s<script type=\"module\">%s</script></body></html>\n", base64.StdEncoding.EncodeToString(hash[:]), escapedTitle, htmlStyle, bodyHTML, htmlRuntime), nil
}

type safeHTMLRenderer struct{}

func (safeHTMLRenderer) RegisterFuncs(reg renderer.NodeRendererFuncRegisterer) {
	reg.Register(ast.KindRawHTML, renderLiteralHTML)
	reg.Register(ast.KindHTMLBlock, renderLiteralHTML)
	reg.Register(ast.KindFencedCodeBlock, renderCode)
}

func renderLiteralHTML(w util.BufWriter, source []byte, node ast.Node, entering bool) (ast.WalkStatus, error) {
	if !entering {
		return ast.WalkContinue, nil
	}
	var value []byte
	switch n := node.(type) {
	case *ast.RawHTML:
		value = n.Segments.Value(source)
	case *ast.HTMLBlock:
		value = n.Lines().Value(source)
		if n.HasClosure() {
			value = append(value, n.ClosureLine.Value(source)...)
		}
	}
	_, err := w.WriteString(html.EscapeString(string(value)))
	return ast.WalkSkipChildren, err
}

func renderCode(w util.BufWriter, source []byte, node ast.Node, entering bool) (ast.WalkStatus, error) {
	if !entering {
		return ast.WalkContinue, nil
	}
	n := node.(*ast.FencedCodeBlock)
	value := html.EscapeString(string(n.Lines().Value(source)))
	info := ""
	if n.Info != nil {
		info = strings.TrimSpace(string(n.Info.Segment.Value(source)))
	}
	if info == "mermaid" {
		_, err := w.WriteString("<figure class=\"mf-diagram\"><p class=\"mf-diagram-status\" aria-live=\"polite\">Mermaid diagram is not rendered yet. JavaScript and CDN access are required; source follows.</p><div class=\"mf-diagram-output\"></div><details open><summary>Diagram source</summary><pre><code>" + value + "</code></pre></details></figure>\n")
		return ast.WalkSkipChildren, err
	}
	class := ""
	if language := n.Language(source); len(language) > 0 {
		class = " class=\"language-" + html.EscapeString(string(language)) + "\""
	}
	_, err := w.WriteString("<pre><code" + class + ">" + value + "</code></pre>\n")
	return ast.WalkSkipChildren, err
}
