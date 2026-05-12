# mf extensions

These commands are mf-only — they have no equivalent in the `mind` CLI. They
provide functionality specific to mf's document-as-code model.

## Index commands

Index commands reconcile the filesystem state with the project index
(`mind-index.yaml`). They detect new, removed, and changed files.

- **`article index`** — scan `docs/` directory and update article index
- **`asset index`** — scan assets directory and update asset index
- **`project index`** — scan projects directory and update top-level manifest
- **`source index`** — scan sources directory and update source index

## Config

- **`config init`** — generate a `mind.yaml` configuration file in the current
  or specified directory
- **`config show`** — display the effective merged configuration (the primary
  target of `config compile`/`generate`/`default` aliases)

## Source

- **`source update <NAME> --rename <NEW> --url <URL>`** — rename a source
  or update its URL

## Term

- **`term new <TERM> --definition <DEF> --alias <ALIAS>`** — create a new
  glossary term entry
- **`term fix <TERM> --definition <DEF> --alias <ALIAS> --tag <TAG>`** —
  update metadata on an existing term

## Global flags

- **`--config <PATH>`** — specify a config file path
- **`--verbose` / `-v`** — increase log verbosity
- **`--quiet` / `-q`** — suppress non-essential output
