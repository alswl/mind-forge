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

### CJK `word` corrections (spec 074 #30)

A `word`-matched CJK correction must fire on an **exact jieba token**. For the
ambiguous short class (≤2 Han characters, e.g. 「以可」), the matched span must be
one contiguous jieba token — a span that merely sits between two separately
emitted tokens (`以` + `可` in 「以可独立验证」) is not flagged. Genuine standalone
short-CJK occurrences still lint, but as **advisory**: `replacement_eligible:
false` with `safety_reason: "short-cjk-advisory"`, so `term fix` never
auto-applies them. Opt in explicitly with `--term <NAME>` or
`--term <NAME:ORIGINAL>` to apply such a finding.

### Held-back corrections (spec 075 #40)

An advisory finding is never silently discarded. `mf term lint` marks it
`, held back` in text output and sets `held_back: true` alongside its
existing `safety_reason` in JSON. `mf term fix` reports a held-back count
next to the applied count and names the scoped remedy:

```text
2 findings in 1 files (1 fixed, 0 failures)
1 finding held back; apply with `mf term fix <PATH> --term <NAME>`
```

The held-back line never appears when nothing was held back, and a
zero-applied result is never reported bare when something was held back
instead of genuinely absent. Apply a held-back finding explicitly with
`--term <NAME>` or `--term <NAME:ORIGINAL>` — the choice channel described
above. No corrections change which ones are applied automatically; this is
purely a reporting completeness fix.

### Longest match wins (spec 075 #41)

When two registered corrections overlap at the same text position — one's
`original` is a prefix of the other's — the longer, more specific
correction always wins, in every mode:

```yaml
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
  - term: Machinery
    corrections:
      - original: 机器人
        correct: 机械装置
```

Linting `机器人` reports only `机器人 → 机械装置`; `机器 → 装置` is
suppressed at that position and never applied, even when a fix is
explicitly scoped to `--term Device`. Scoping to `--term Machinery` applies
the longer correction. The shorter correction still fires normally when it
appears standing alone, outside any longer match. Exact ties (same span
length) break by declaration order in `mind-index.yaml`.

### Cross-term shadowing warning (spec 075 #42)

Registering a correction whose `original` is a prefix of — or equal to —
another term's name or one of its registered originals warns and names the
shadowed term, but still registers:

```text
$ mf term correction add Widget widget Widget --project alpha
warning: original 'widget' is also a prefix of term 'Widget Cloud'; lint may map it to that term instead
```

A lint finding whose original could be claimed by more than one term
discloses the competing term(s) so the misdirection is diagnosable from the
finding alone, without inspecting every term individually:

```text
docs/t.md:1:1: "机器" → "装置" [Device], standalone, held back
  also claimed by: Machinery
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
