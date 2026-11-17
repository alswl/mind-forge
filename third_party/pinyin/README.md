# Fixed first-reading pinyin data

`internal/text/data/pinyin-first.tsv.gz` contains each Unicode scalar's first
toneless reading as returned by the frozen Rust `pinyin` 0.10.0 crate. Unknown
scalars have no row. It preserves the original library's reading order, `ü`,
rare-character membership and context-free conversion, without adopting another
library's current dictionary or fallback defaults.

The source crate uses [mozillazg/pinyin-data
0.13.0](https://github.com/mozillazg/pinyin-data/tree/v0.13.0). Both the crate and
dataset are MIT licensed by mozillazg; their identical copyright and license
notice is included as `LICENSE`. The dataset's version and source header were
checked in the frozen crate's `pinyin-data/pinyin.txt`; the dataset's own license
was checked at its versioned upstream URL.

The external owner component oracle is
`my-tests/apps/mf/compat/text-oracle`. It writes a static table and expected
Unicode digest. Product unit tests read fixed inputs directly, without invoking
the oracle, a CLI or Rust. See `internal/text/testdata/provenance.json` for source,
compiler, executable, dictionary and compressed-data identities. This component
reference does not supply backing for external business acceptance.
