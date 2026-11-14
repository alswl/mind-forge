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
