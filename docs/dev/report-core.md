# Report engine, Core and API verification

`make build build-report` builds `bin/mf` and `bin/mf-report` with the
repository's version stamp. `make check` also builds both. The same React engine,
built-ins, and starter are embedded in both executables. Bring Node 24 and use
the scaffolder's locked dependency install command. Listing, scaffolding,
and dry-run work without Node. The report engine requires no SDK installation.

## CLI

| Command | Report options |
| --- | --- |
| `mf report init SLUG` | `--renderer ID` |
| `mf report build SLUG` | `--renderer ID`, `--view chart\|table`, `--theme system\|light\|dark`, `--worker PATH`, `-n/--dry-run` |
| `mf report renderer list`, `mf report component list` | `--id ID` |
| `mf report catalog build SLUG` | `--theme`, `--worker PATH` |
| `mf report serve` | `--listen ADDR`, `--worker PATH` |
| `mf report openapi` | Writes YAML to stdout |
| `mf-report build` | Same build options, plus optional paired `--entry PATH --data PATH` in place of `report.yaml` |
| `mf-report list renderers\|components` | `--id ID` |
| `mf-report catalog build` | `--theme`, `--worker PATH` |
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

The default worker invokes Node 24 from PATH with an unpacked, embedded
script. It resolves React, React DOM, and esbuild from the report workdir's
locked dependencies. Go removes the private temporary engine and staged
output on failure or cancellation. Operator-only `--worker` overrides the engine
with an executable path, resolved relative to the caller or given as an
absolute path. It receives no arguments. Either process uses the absolute
report directory as its current directory. Execute trusted source with the
invoking user's permissions.

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
| `artifact` | Expected workdir-relative destination, `outputs/report.<view>.html` or `outputs/catalog.html` |
| `catalog` | Optional array of component modules plus `description` and `example`. Present only for catalog builds. |

A module's `entry` is an absolute path for repository modules and
an `embedded:…` identifier for built-ins. Its `schema` is the inline JSON Schema document.
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

## Embedded renderer and browser reproduction

Use Node 24, Corepack and a project scratch directory on a supported local
filesystem. With containers on macOS, copy the synthetic project to a container
tmpfs before running `mf report init` so the repo write lock uses a supported
filesystem. Build the CLI for the platform where you will run it.

```sh
make build build-report
qa=$(mktemp -d)
cp -R pkg/services/report/testdata/workspace "$qa/repo"
mkdir -p "$qa/repo/.mind-forge"
cp -R pkg/services/report/testdata/registry "$qa/repo/.mind-forge/report"
bin/mf --root "$qa/repo" -p lab report init january --renderer builtin/trend --json

for slug in january weekly; do
  report_dir="$qa/repo/projects/lab/templates/reports/$slug"
  if [ "$slug" = weekly ]; then
    cp pkg/services/report/starter/package.json pkg/services/report/starter/pnpm-lock.yaml "$report_dir/"
  fi
  (cd "$report_dir" && corepack pnpm@10.28.2 install --frozen-lockfile)
  bin/mf --root "$qa/repo" -p lab report build "$slug" --view chart --json
  bin/mf-report --workdir "$report_dir" --registry-root "$qa/repo" build --view table --json
done
bin/mf --root "$qa/repo" -p lab report catalog build weekly --theme dark --json
```

Open each file alone from a fresh directory via `file://` with the browser
offline. For the January report, compare the order and labels of the chart
tooltips to the semantic table. January 5 has 0 completed requests, January 12
has an explicitly missing completion count, and January 19 has 7. On the chart,
focus the plot then tab to a mark or point and press Escape to close its
tooltip. Try a 375 px viewport, light and dark system preferences, and disabled
JavaScript. At narrow widths plots can scroll within their own region. Table
HTML omits the plot and its enhancement script. In `weekly/outputs/catalog.html`,
open `repo/delivery`'s schema and example and copy the template. Verify that the
custom CSS and PNG render in both report views and the catalog without network
requests. Ordinary source links may be present and load only when followed.

Build with an empty PATH to exercise missing Node while keeping the binaries
addressable by absolute path. The CLI exits nonzero, HTTP exports return 503
Problem Details, and existing artifacts retain their bytes. A renderer or
component validation failure returns 422 through HTTP and leaves prior output
intact. List and dry-run still work.

## Release staging

`make check` verifies hashes of embedded assets and their inputs. Run
`make check-report-source` under Node 24 to typecheck, test and reproduce those
assets from the frozen lock. It uses a scratch directory under `bin/`.
For an offline run point `REPORT_PNPM_STORE` at a preloaded store and set
`REPORT_PNPM_OFFLINE=1` with a preloaded `COREPACK_HOME`. To regenerate after
editing TypeScript, stage the same `report/` inputs and run
`REPORT_REPO=/absolute/checkout node generate.mjs` from the staged directory.

```sh
make check
make package PACKAGE_OUT="$PWD/bin/local-report-release"
(cd bin/local-report-release && shasum -a 256 -c checksums.txt)
for archive in bin/local-report-release/*.tar.gz; do
  tar -tzf "$archive"
done
```

Use a new staging path from a clean checkout. Each archive has two version-matched
platform executables and the existing name. The archive carries the engine
inside those executables. The source installer also installs both commands.
Node and the report workdir dependencies are supplied separately. The real
release job requires an authorized tag which matches VERSION at a clean HEAD.

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

Start the server and copy the URL and bearer token from stderr. The example uses
the controlled worker. Remove the override after installing the fixture's
locked dependencies to exercise the real renderer. The default
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
