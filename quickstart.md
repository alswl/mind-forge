# mind-forge quickstart

This walkthrough uses only local files and the offline search index.

```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes
mf article new "First Note" --project notes
printf '# Reference\n\nA local reference.\n' > reference.md
mf source new ./reference.md --project notes
mf source sync --offline
mf search "local reference" --output json
mf article index --project notes
mf build docs/first-note --project notes
```

The repository keeps human-edited configuration in `minds.yaml` and
rebuildable RAG state under `.mind-forge/`. Use `mf source sync --offline` to
rebuild that local state after a cache loss.
