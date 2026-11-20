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
`--register-only` registers a file already inside the project's sources directory
in `mind-index.yaml` without copying it or creating a semantic index. For
meeting notes, run the preview first and then register the file:

```bash
mf source new sources/meeting/notes.md --register-only --source-kind meeting -p notes --dry-run
mf source new sources/meeting/notes.md --register-only --source-kind meeting -p notes
```

Remove `--no-index` from older archive scripts. Mind Forge removed the flag and
the built-in semantic index in v0.5.2.

Use `list` and `show` to locate evidence, then read the saved file. Search the
repository with your editor or file search tools. Verify factual claims against
Source content and preserve its original URL or path when citing it.

`source index` adopts new files and counts existing sources separately from
registrations whose files are missing. Text output labels the latter
`missing (retained)`. In JSON, the historical `removed` list reports those
same missing files. Their registrations stay in the index. Use `source clean`
or `source remove` for explicit removal. The `source remove --keep-file`
option preserves the file while removing its registration.

`mf source rename <old> <new>` treats `<new>` as a cwd-relative or absolute path.
To rename in place, include its directory and extension, for example:
`mf source rename reference projects/notes/sources/file/renamed.md --project notes`
from the repository root.

Source/Asset ownership and lifecycle moves follow the
[content layout](../manual/content-layout.md) and [lifecycle](manage-content.md)
rules.
