# Report Core and API verification

`make build build-report` builds `bin/mf` and `bin/mf-report` with the
repository's version stamp. `make check` also builds both. The Core can discover,
plan, validate, and run reports with a controlled executable. The React engine,
the offline component directory, and the version-matched npm dependencies are
pending. The scaffolder supplies an editable entry, a sample, and an empty pnpm
starter lockfile. It performs no installation.

## CLI

| Command | Report options |
| --- | --- |
| `mf report init SLUG` | `--renderer ID` |
| `mf report build SLUG` | `--renderer ID`, `--view chart\|table`, `--theme system\|light\|dark`, `--worker PATH`, `-n/--dry-run` |
| `mf report renderer list`, `mf report component list` | `--id ID` |
| `mf report serve` | `--listen ADDR`, `--worker PATH` |
| `mf report openapi` | Writes YAML to stdout |
| `mf-report build` | Same build options, plus optional paired `--entry PATH --data PATH` in place of `report.yaml` |
| `mf-report list renderers\|components` | `--id ID` |
| `mf-report serve` | `--listen ADDR`, `--worker PATH` |
| `mf-report openapi` | Writes YAML to stdout |

`mf` uses its existing `--root`, `-p/--project`, runtime configuration, and
current-directory selection. Its reports live in the selected project's
`<layout.templates>/reports/SLUG`. The standalone CLI uses `--workdir` (current
directory by default) and optional `--registry-root` (the directory containing
`minds.yaml`). Its server uses the working directory's basename as the only
report slug and reads its `report.yaml`. Explicit standalone inputs default to
`builtin/brief`. Both commands accept `--json`, `-o/--output`, and `-q/--quiet`.
The defaults are view `chart` and theme `system`.

Every file manifest uses `apiVersion: report.mind-forge/v1` and the matching
`kind` (`Report`, `Renderer`, or `Component`). Renderer and component manifests
require a semantic `version`. Report manifests can include one. Renderer and component directories
under `.mind-forge/report/{renderers,components}` match the final segment of
`repo/SLUG` exactly. Slugs are lowercase ASCII letters and digits separated by
single hyphens, with a limit of 100 characters. Relative paths are case
sensitive. Manifest and report paths reject parent traversal and symlinks.
Schemas support local fragments and embedded JSON Schema resources. External
schema loads are disabled. List returns sorted metadata and diagnostics.
Build validates the selected renderer's declared dependencies, parses the
selected data as JSON, and validates it against the renderer's data schema.
Dry-run resolves the manifest, schema, paths, and dependencies without reading
the report data, creating output files, or starting the worker.

## Worker protocol

The host invokes one executable directly with no arguments, using the absolute
report directory as its current directory. `--worker` selects an executable
relative to the caller's directory or by absolute path. Its default is the
working directory's `node_modules/.bin/mind-forge-report-worker`.
Use a worker and report sources you trust. The worker runs with the invoking
user's permissions.

The host sends a UTF-8 JSON object followed by a newline on stdin, then closes
stdin. The worker checks `protocol` before processing input.

| Request field | Value |
| --- | --- |
| `protocol` | `report.mind-forge/v1` |
| `workDir`, `entry`, `data` | Absolute working directory, report entry path, and data file path |
| `renderer` | Object with `id`, `version`, `entry`, and `schema` |
| `components` | Array of the same module objects, in manifest order |
| `view`, `theme` | Resolved view and theme |
| `stage` | Absolute path to an existing private temporary file in `outputs` |
| `artifact` | Expected workdir-relative destination, `outputs/report.<view>.html` |

A module's `entry` is an absolute path for repository modules and a package
specifier for built-ins. Its `schema` is the inline JSON Schema document.
The worker writes nonempty output into the supplied staging file, keeping its
file identity. On success, it exits zero and sends exactly one JSON receipt on
stdout. This receipt is for a table export of the included fixture.

```json
{
  "protocol": "report.mind-forge/v1",
  "stage": "/absolute/workdir/outputs/.mf-report-unique.tmp",
  "result": {
    "protocol": "report.mind-forge/v1",
    "renderer": "repo/weekly",
    "rendererVersion": "1.0.0",
    "view": "table",
    "artifact": "outputs/report.table.html"
  }
}
```

Echo the actual stage path and all result fields exactly. Stdout has a 64 KiB
receipt limit. Send diagnostics to stderr and use nonzero exit status when the
worker cannot complete. The host validates the receipt and staging file,
replaces the destination by same-directory rename, and removes the stage on
failure or cancellation. Parallel exports have distinct staging filenames.
If two successful exports target the same view, the last rename determines the
visible artifact. Cancellation terminates the process group on macOS and Linux.

## Controlled fixture

Run from the source checkout. Keep the scratch directory for the HTTP steps.

```sh
make build build-report
mf_qa=$(mktemp -d)
go build -o "$mf_qa/worker" ./pkg/services/report/testdata/worker
cp -R pkg/services/report/testdata/workspace "$mf_qa/repo"
mkdir -p "$mf_qa/repo/.mind-forge"
cp -R pkg/services/report/testdata/registry "$mf_qa/repo/.mind-forge/report"
report_dir="$mf_qa/repo/projects/lab/templates/reports/weekly"

bin/mf --root "$mf_qa/repo" report renderer list --id repo/weekly --json
bin/mf --root "$mf_qa/repo" -p lab report build weekly --view table --dry-run --json
bin/mf --root "$mf_qa/repo" -p lab report build weekly \
  --view table --worker "$mf_qa/worker" --json
cp "$report_dir/outputs/report.table.html" "$mf_qa/table.html"

bin/mf-report --workdir "$report_dir" --registry-root "$mf_qa/repo" build \
  --view table --worker "$mf_qa/worker" --json
cmp "$mf_qa/table.html" "$report_dir/outputs/report.table.html"

bin/mf --root "$mf_qa/repo" -p lab report init scratch --renderer repo/weekly
bin/mf report openapi > "$mf_qa/openapi.yaml"
```

Repeat the table build with `MF_REPORT_FIXTURE_MODE=fail`, `bad-protocol`,
`bad-artifact`, or `extra-stdout` set on the host command. It should fail,
`cmp` should still match, and `outputs` should contain no
`.mf-report-*.tmp` files. With mode `wait`, the worker writes
`fixture staged` to stderr after touching the staging file. Cancel at that
point to exercise cleanup and preservation. With no mode, the process generates
deterministic synthetic HTML from the request and data for both view values.

## HTTP

Start the server and copy the URL and bearer token from stderr. The default
bind is `127.0.0.1:8787`. Port `0` asks the OS for an available port. The
listen host must be a loopback IP or `localhost`.

```sh
bin/mf --root "$mf_qa/repo" -p lab report serve \
  --listen 127.0.0.1:0 --worker "$mf_qa/worker"
```

Set `url` and `token` to those printed values before running the requests.

```sh
curl -fsS "$url/health"
curl -fsS "$url/ready"
curl -fsS "$url/v1/renderers?id=repo%2Fweekly"
curl -fsS "$url/v1/components"
curl -fsS "$url/openapi.yaml"
curl -fsS "$url/v1/reports/weekly/exports" \
  -H "Authorization: Bearer $token" -H 'Content-Type: application/json' \
  --data '{"view":"table"}'
```

The response to the POST equals the CLI envelope's `data` for the same
export. The optional body accepts only `renderer`, `view`, and `theme`.
Compare the saved HTML to the new table artifact with `cmp`. Interactive
API documentation is at `/docs`, and the OpenAPI 3.1 contract is also at
`/openapi.json`. An export needs the startup token and a local Host.
When an Origin header is present, it must match the request's local HTTP
origin. Missing or invalid bearer credentials return 401. Host or Origin
mismatch returns 403. Unknown reports or IDs return 404. Validation
failures return 422 and worker failures return 503, with Problem Details.
Stopping the server cancels in-flight builds.
