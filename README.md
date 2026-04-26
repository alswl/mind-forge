# mind-forge

Rust CLI framework for the `mf` command tree.

## Current scope

This repository currently implements the framework stage of `mf`:

- multi-level command tree for `source`, `asset`, `project`, `article`, `term`
- shared global flags such as `--config`, `--verbose`, `--quiet`, `--format`, `--no-color`
- consistent placeholder responses for leaf commands
- shell completion generation via `mf completion <shell>`

## Quickstart

```bash
cargo test
cargo run -- --help
cargo run -- source list
cargo run -- completion zsh
```
