# Frozen Unicode text behavior

`internal/textcase/lowercase_data.go` contains Unicode 17.0.0 mappings observed
with Rust 1.98.0 (`88d9e12ae`), the compiler used for the migration reference.
It keeps saved-plan path collision checks independent of Go's Unicode version.
The implementation includes the language-independent final-sigma rule from
Rust's `alloc/src/str.rs`; the remaining mappings are context independent.

The generated data includes all 1,488 nonidentity scalar lowercase mappings.
The two range tables classify characters for final-sigma context. Characters
that are both case-ignorable and cased belong to the ignorable table, because
the context rule skips them before inspecting the next character's case.
The 5,567 frozen string fixtures were evaluated with Rust's `str::to_lowercase`.
Rust is used only to generate these migration fixtures and data; the Go binary
does not call Rust or load external tables.

Unicode data is covered by `LICENSE-UNICODE`; the Rust-derived algorithm is
available under `LICENSE-MIT` or `LICENSE-APACHE`. Migration provenance and the
oracle source are retained with the feature's private migration records.

`internal/textcase/alphanumeric_data.go` additionally freezes the 844
alphanumeric ranges observed by enumerating all Unicode scalar values with
that same Rust compiler (`char::is_alphanumeric`). Article/block filename
normalization reuses these ranges, including alphabetic combining marks and
characters introduced after Go standard-library Unicode 15.
