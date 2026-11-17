# Legacy Article quickstart

This walkthrough uses local files. It creates
a legacy project without `content_layout`. For new category/bundle projects, use
[minds and outputs](docs/guide/minds-and-outputs.md).

```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes
mf article new "First Note" --project notes
printf '# Reference\n\nA local reference.\n' > reference.md
mf source new ./reference.md --project notes
mf article index --project notes
mf build docs/first-note --project notes
```

The repository keeps configuration in `minds.yaml`, Source registrations in
`mind-index.yaml`, and authored content in ordinary files.

## Go developer verification

The product checkout is self-contained for local checks; it does not need the
external `my-tests` repository. Select Go 1.26.11 from `.tool-versions`, then
run the full formatter, vet, unit-test and build gate:

```bash
go version
bash scripts/check.sh
bash scripts/generate-manual.sh
git diff --exit-code -- docs/manual/cli.md
```

`scripts/check.sh` builds the stamped `bin/mf`. Manual generation uses that
binary and must leave the checked-in manual unchanged. CLI, recovery, install
and platform acceptance still run through the separately pinned external
verification harness.
