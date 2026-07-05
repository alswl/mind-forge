# Term Lint

`mf term lint` scans project documents for term inconsistencies and, with `--fix`, rewrites them in place.

## Correction Fields

Each correction in `mind-index.yaml` supports these fields:

| Field | Values | Default | Description |
|-------|--------|---------|-------------|
| `original` | string | _(required)_ | The variant text to find |
| `correct` | string | _(required)_ | The canonical replacement |
| `match` | `word`, `substring`, `pinyin` | `word` | Match strategy |
| `fix` | `required`, `suggested` | `required` | Whether `--fix` rewrites automatically |
| `boundary` | `loose`, `standalone` | `standalone` | Match-boundary policy (see below) |

## Boundary Field

The `boundary` field controls what characters may appear next to a match.

### `loose`

For `substring`, performs literal matching at any position. For `word`, preserves
the existing word-boundary behavior.

### `standalone`

The safe default. For ASCII, the match is **rejected** when either neighbour
belongs to `{ letter, digit, _ - / \ . }`. For CJK substring corrections, both
edges must align with jieba token boundaries. This means:

```text
xxx-aidc-test          → skipped (hyphen neighbours, identifier-internal)
./docs/aidc/intro.md   → skipped (slash neighbours, path-internal)
my_aidc_db             → skipped (underscore neighbours, snake_case)
独立 aidc 站点          → matched (whitespace neighbour on both sides)
```

### When to Use

Use `boundary: standalone` when a short ASCII acronym (`aidc`, `ob`, `ats`) was previously demoted to `fix: suggested` because of identifier-collision risk. Pairing `boundary: standalone` with `fix: required` restores automatic rewriting while respecting identifier boundaries.

### Setting via CLI

```bash
mf term update AIDC --correction-boundary aidc:standalone
```

### Setting via YAML

```yaml
terms:
  - term: AIDC
    corrections:
      - original: aidc
        correct: AIDC
        boundary: standalone
```

The field is omitted on serialization when `standalone` (the default).

## Validation Errors

Invalid identifier-edge combinations produce an error (exit code 2):

| Condition | Message |
|-----------|---------|
| `standalone` with `original` starting/ending in `-` or `_` | `boundary: standalone cannot apply to identifier-character edges` |

## Migration Playbook

Terms previously demoted to `fix: suggested` because their corrections matched inside identifiers can be promoted back:

```diff
  - original: aidc
    correct: AIDC
    match: word
-   fix: suggested
+   fix: required
+   boundary: standalone
```
