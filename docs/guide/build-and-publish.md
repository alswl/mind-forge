# Build and publish an output

Select an editable Output with its full identity, such as
`projects/research::brief::summary`. Markdown is the default build format.
For HTML templates and diagram fallback, see [HTML output](build-html.md).
The `first-note` examples below refer to a [legacy Article](legacy-articles.md).


`mf build` merges blocks in filename order and rewrites relative links so they
resolve from the output directory. It can also run an asset pipeline declared
in `mind.yaml`:

```yaml
build:
  strip_first_h1: true
  pipeline:
    - name: d2-to-svg
      input_extension: d2
      output_extension: svg
      command: "d2 {input} {output}"
    - name: svg-to-png
      input_extension: svg
      output_extension: png
      command: "rsvg-convert {input} --output {output}"
```

Only missing or stale outputs are rebuilt, and rebuilding a stage also
rebuilds the stages after it. If an optional tool is missing or fails, `mf`
prints a warning and leaves existing outputs in place. `mf build <article>
--dry-run` prints the plan without running anything.

## Private content

Private content stays in the authored files and is stripped during assembly,
so no build or publish target receives it.

| Marker | Effect |
|---|---|
| `> [!mf-private]` / `> [!mind-forge-private]` callout | The callout is removed (markers inside fenced code are kept as examples) |
| Block front matter `mind-forge-visibility: private` | The whole block is skipped |

> [!IMPORTANT]
> The build fails rather than emit a titleless artifact if the first/title block
> is private, or if `mind-forge-visibility` is anything other than `public` or
> `private`. `mf article lint` reports both cases.

## Publish

```yaml
# projects/notes/mind.yaml
publish:
  default_target: local
  targets:
    - name: local
      type: local
      path: ../../published
```

```bash
mf publish run first-note --project notes --dry-run
mf publish run first-note --project notes
```

| Target type | Behavior |
|---|---|
| `local` | Copies the built article to `path` (relative paths resolve from the project directory; honors `config.prefix`) |
| `yuque-prompt` | Writes a persistent prompt file for an agent to publish to Yuque |

Without `--target`, `mf` uses `publish.default_target`. File-based publishers in
`.mind-forge/publisher/<name>.yaml` are discovered as well. Front matter is kept
by default; set `publish.strip_front_matter: true` or pass
`--strip-front-matter` for one run (`--keep-front-matter` overrides the
project setting).

`mf render <article> --template report` writes a render prompt instead of a
file. It is meant to be handed to an agent.

## Output paths and local publishing

In category/bundle projects, build products use `build/<mind>/<output>.md|html`.
Local publication preserves the Mind/Output path to avoid same-name collisions.
A destination inside the project must remain under `build/`.
Publication requires an existing Markdown build and does not build automatically.
See the [layout reference](../manual/content-layout.md) for selection and path constraints.
