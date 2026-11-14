# Manage sources

Sources preserve evidence and provenance in local files. The project's
`mind-index.yaml` stores registrations, including each Source's name, path,
original URL, tags, and timestamps.

```bash
mf source new ./reference.md --project notes
mf source new https://example.com/reference --name reference --project notes
mf source list --project notes
mf source show sources/file/reference.md --project notes
mf source index --project notes
mf source clean --project notes --dry-run
```

URL imports download and save the original bytes under `sources/web/` or
`sources/rss/`. Local imports copy a file; `--link` creates a symlink and
`--register-only` registers a file already inside the project's sources directory.

Use `list` and `show` to locate evidence, then read the saved file. Search the
repository with your editor or file search tools. Verify factual claims against
Source content and preserve its original URL or path when citing it.

`source index` adopts new files and reports missing registrations without deleting
them. Use `source clean` or `source remove` for explicit removal. `--keep-file`
removes a registration while preserving its file.

`mf source rename <old> <new>` treats `<new>` as a cwd-relative or absolute path.
To rename in place, include its directory and extension, for example:
`mf source rename reference projects/notes/sources/file/renamed.md --project notes`
from the repository root.

Source/Asset ownership and lifecycle moves follow the
[content layout](../manual/content-layout.md) and [lifecycle](manage-content.md)
rules.
