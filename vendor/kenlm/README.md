# Vendored KenLM Query-Only Sources

Upstream: https://github.com/kpu/kenlm
Pinned commit: `4cb443e60b7bf2c0ddf3c745378f76cb59e254e5`
Date: 2026-06-28

## Included files

The `lm/` and `util/` directories contain the minimal source subset
required for query-only operations (model load, sentence score,
perplexity, vocabulary lookup, OOV check). Build system, training,
filtering, fragment, and benchmark sources are excluded.

The C ABI shim (`mf_kenlm_shim.h` / `mf_kenlm_shim.cc`) exposes a
narrow query surface used by the safe Rust wrapper in
`src/service/term/correct/kenlm_ffi.rs`.

## Project-owned stub files

Two minimal `.cc` files provide linker stubs for build-time symbols
that are referenced by the vendored query sources but not needed at
query time:

- `lm/bhiksha.cc` — `DontBhiksha` / `ArrayBhiksha` constructors and
  static methods (no-op implementations).
- `util/ersatz_progress.cc` — `ErsatzProgress` constructor, destructor,
  and `Milestone()` (no-op; progress reporting is not used at query
  time).

These stubs avoid pulling in additional KenLM sources that are only
relevant for model building.

## Building

Compiled by `build.rs` via the `cc` crate with C++17. The shim only
exposes query functions — no model creation, training, or I/O beyond
loading an existing binary or ARPA model.

## License

KenLM is distributed under the LGPL 2.1+. See the upstream repository
for details. The vendored subset is used as a query-only library in a
private local experiment.
