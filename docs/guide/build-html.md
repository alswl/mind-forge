# HTML output

`mf build` assembles one editable output, removes private content, rebases its
resource paths, and renders Markdown (the default) or HTML. It does not start a
server or publish anything.

```sh
mf build projects/research::brief::design
mf build projects/research::brief::design --format html
mf build projects/research::brief::design --format html --output json
mf build projects/research::brief::design --format html --dry-run
```

The artifact format flag is independent of the global JSON/text envelope flag.
Defaults are `build/brief/design.md` and `build/brief/design.html`; different outputs
retain different paths. `--out` still obeys source-overlap and project build-root
checks. Zero/multiple-output selection follows the existing resolver: multiple
outputs need an explicit identity or configured default. Root and nested projects,
category and bundle layouts, single files and ordered blocks use the same renderer.

HTML supports headings, paragraphs, lists, links, tables, fenced code, local images,
and Mermaid fences. Images use the existing rebased resource paths; assets are not
bundled or copied automatically. The output page needs access to those resources.
Prompt, Thinking and additional content are not included. Private blocks and
callouts are removed before HTML rendering, including from comments and scripts.
An initial private title block remains an error, as in Markdown builds.

## Project template

Configure a project-relative static body template in `mind.yaml`:

```yaml
build:
  html_template: templates/page.html
```

```html
<header class="project-header">Research notes — {{title}}</header>
<main class="mf-content">{{content}}</main>
<footer>Prepared for review</footer>
```

Exactly one `{{content}}` placeholder must be a standalone text node. `{{title}}`
is optional and contains the selected identity/name, escaped as text. The renderer
owns the document head, basic stylesheet, CSP and Mermaid runtime. Template HTML
is safety-filtered; scripts, event handlers, unsafe URLs and active embeds do not
execute. The template is a static body fragment, not a plugin or script interface.
Missing/invalid templates fail before artifact writes; invalid placeholder shape
returns `invalid_html_template`. Dry-run validates the template without creating
files, locks, journals or caches or executing an asset pipeline.

## Mermaid and failure behavior

Mermaid is loaded from the fixed official package URL:
`https://cdn.jsdelivr.net/npm/mermaid@11.12.0/dist/mermaid.esm.min.mjs`.
The browser renders with strict security, disables HTML labels for flowcharts,
and rejects diagram configuration directives/front matter. Raw document HTML is
shown as text; generated Markdown and the substituted template are sanitized.
A CSP also restricts scripts to the built runtime and that Mermaid package version.

Every diagram retains its escaped source in an expandable **Diagram source** block.
Loading failure or a 15-second load timeout displays **Mermaid could not load**.
Invalid syntax or forbidden configuration displays **Mermaid diagram could not be
parsed or uses unsupported configuration**. In both cases the source stays visible;
other content, images and diagrams remain readable. Without JavaScript, the initial
message explains the requirement and the source is already visible.

CDN access is a supported runtime prerequisite. Offline bundles, formula rendering,
footnotes, theme/plugin suites and automatic HTTP hosting are outside this feature.
