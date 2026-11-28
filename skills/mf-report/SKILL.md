---
name: mf-report
description: Build a data report or an offline HTML export with Mind Forge. Use when selecting charts, exporting chart and table views, extending the report renderer or component registry, or rebuilding the offline component catalog.
---

# Mind Forge data reports

1. Resolve the Mind Repo and project using the actual `mf` CLI. Read
   `mf report renderer list --json` and `mf report component list --json`.
   Inspect the selected ID's schema and example. Reuse a registered renderer
   when it fits the reader's question. Add a repo component when only one
   building block is missing.
2. Confirm the reader's question, units, time range, denominator, missing
   values and source. Use author-approved prose and data for delivery. Keep
   zero distinct from missing and supply explicit nulls where allowed. Use
   [visualization choices](references/reports.md) for the report contract and
   extension example.
3. Use `mf report init SLUG -p PROJECT --renderer ID --json` for a new report.
   Read the returned directory and install command. Node 24 and the project's
   locked dependencies are needed for export. Both Mind Forge binaries already
   include the report engine.
4. Validate scope and selected dependencies with
   `mf report build SLUG -p PROJECT --dry-run --json`. Export chart and table
   with `--view chart` and `--view table`. Compare labels, displayed values,
   units and missing observations. Open each HTML by itself from an empty
   directory with network access disabled. Verify keyboard focus, narrow
   layout, legibility in both themes, and the content with scripts disabled.
   If a browser check is unavailable, state the actual limitation.
5. Rebuild `mf report catalog build SLUG -p PROJECT --json` after extending
   the registry and verify the new sample and props schema there. Return the
   CLI-reported renderer ID and actual paths, which are relative to the report
   working directory. A Markdown revision stays on the mf-plan or mf-write
   workflow unless the author requests a data report.
