# Fixed jieba text model

`pkg/common/text/data/dict.txt.gz` is the unchanged UTF-8 default dictionary from
the frozen `jieba-rs` 0.7.4 crate. `hmm.model.gz` is the unchanged model from
its `jieba-macros` 0.7.1 dependency. Both belong to
[messense/jieba-rs](https://github.com/messense/jieba-rs/tree/v0.7.4), released
under the included MIT license. The Go DAG/HMM implementation is adapted from
that project's `src/lib.rs` and `src/hmm.rs`, under the same notice.

Compression uses gzip with zero modification time. Uncompressed and compressed
SHA-256 identities are recorded in `pkg/common/text/testdata/provenance.json`.
These models are embedded; no installed dictionary, download or Rust runtime is
used. Updates require regenerating the component reference fixtures and reviewing
token boundaries, HMM decisions and tie-breaking against the frozen contract.
