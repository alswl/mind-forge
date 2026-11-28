import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import { build } from "esbuild";

const root = resolve(process.env.REPORT_REPO ?? "..");
const service = "pkg/services/report";
const check = process.argv.includes("--check");
const output = new Map();
const bundle = async (entry, name, external = []) => {
  const result = await build({
    entryPoints: [entry], outfile: name, bundle: true, platform: "node", target: "node24",
    format: "cjs", external, write: false, minify: true, legalComments: "eof",
    jsx: "automatic", loader: { ".css": "text", ".txt": "text" },
  });
  output.set(`${service}/engine/${name}.gz`, gzipSync(result.outputFiles[0].contents, { level: 9 }));
};
await bundle("src/build/worker.ts", "worker.cjs");
await bundle("src/renderers/builtins.tsx", "builtins.cjs", ["react", "react/jsx-runtime"]);

const descriptions = {
  metric: "A labeled value with a unit and optional context",
  document: "The report title, subtitle and document content",
  narrative: "A short narrative section",
  table: "The observations in a semantic table",
  line: "A trend across dates or an ordered dimension, with gaps for missing observations",
  bar: "Grouped category bars with a zero baseline",
  scatter: "An XY plot with a numeric horizontal dimension",
  "stacked-share": "Shares using a stated denominator without redistributing missing values",
};
const kind = Object.keys(descriptions).slice(3);
const text = { type: "string" };
const number = { type: ["number", "null"] };
const chart = {
  type: "object",
  required: ["title", "rows", "x", "series"],
  properties: {
    title: text, rows: { type: "array", items: { type: "object" } },
    x: { type: "object", required: ["key", "label"], properties: {
      key: text, label: text, type: { enum: ["category", "number", "date"] }, unit: text, timeZone: text,
    } },
    series: { type: "array", minItems: 1, items: { type: "object", required: ["key", "label"],
      properties: { key: text, label: text, unit: text } } },
    rangeLabel: text, denominator: text, missingLabel: text,
  },
};
const metric = { type: "object", required: ["label", "value"],
  properties: { label: text, value: number, unit: text, note: text, missingLabel: text } };
const data = {
  type: "object", required: ["title"],
  properties: {
    title: { type: "string", minLength: 1 }, subtitle: text, narrative: text, rangeLabel: text,
    metrics: { type: "array", items: metric },
    charts: { type: "array", items: {
      ...chart, required: [...chart.required, "kind"],
      properties: { ...chart.properties, kind: { enum: kind } },
    } },
    rows: chart.properties.rows, x: chart.properties.x, series: chart.properties.series,
  },
  dependencies: { rows: ["x", "series"], x: ["rows", "series"], series: ["rows", "x"] },
};
const views = ["chart", "table"];
const base = (kind, slug, description) => ({
  apiVersion: "report.mind-forge/v1", kind, id: `builtin/${slug}`, version: "1.0.0",
  entry: `embedded:${kind.toLowerCase()}/${slug}`, description, source: "builtin",
});
const schemas = {
  metric,
  document: { type: "object", required: ["title"], properties: { title: text, subtitle: text } },
  narrative: { type: "object", required: ["text"], properties: { title: text, text } },
  "stacked-share": { ...chart, required: [...chart.required, "denominator"] },
};
const components = Object.entries(descriptions).map(([slug, description]) => ({
  ...base("Component", slug, description), schema: schemas[slug] ?? chart, example: `embedded:example/${slug}`,
}));
const rows = [
  { week: "2026-01-05", group: "Intake", done: 0, forecast: 3 },
  { week: "2026-01-12", group: "Progress", done: null, forecast: 5 },
  { week: "2026-01-19", group: "Delivery", done: 7, forecast: 8 },
];
const series = [{ key: "done", label: "Completed", unit: "requests" }, { key: "forecast", label: "Forecast", unit: "requests" }];
const example = { title: "Delivery through January", subtitle: "January 5 to January 19, 2026",
  narrative: "Delivery resumed in the third week. The second week has no reported completion count.",
  metrics: [{ label: "Requests completed", value: 7, unit: "requests", note: "Reported weeks only" }] };
const renderers = [
  ["brief", "A short report with narrative, metrics and selected evidence", { ...example,
    charts: [{ kind: "bar", title: "Requests by group", x: { key: "group", label: "Group" }, series, rows }] }],
  ["comparison", "Compare categories with grouped bars and a shared table", { ...example,
    x: { key: "group", label: "Group" }, series, rows }],
  ["trend", "Describe change over time with a gap for missing observations", { ...example,
    x: { key: "week", label: "Week starting", type: "date", timeZone: "UTC" }, series, rows }],
].map(([slug, description, exampleData]) => ({
  ...base("Renderer", slug, description), schema: data, exampleData, views, components: components.map(item => item.id),
}));
output.set(`${service}/assets/catalog.json`, Buffer.from(JSON.stringify([...components, ...renderers], null, 2) + "\n"));

async function sources(dir) {
  const entries = await readdir(join(root, dir), { withFileTypes: true });
  const nested = await Promise.all(entries.map(item => item.isDirectory()
    ? sources(`${dir}/${item.name}`) : `${dir}/${item.name}`));
  return nested.flat();
}
const inputs = [...await sources("report/src"), ...await sources(`${service}/starter`),
  "report/package.json", "report/pnpm-lock.yaml", "report/tsconfig.json", "report/generate.mjs", ...output.keys()].sort();
const manifest = await Promise.all(inputs.map(async path => {
  const sha = createHash("sha256").update(output.get(path) ?? await readFile(join(root, path))).digest("hex");
  return `${sha}  ${path}\n`;
}));
output.set(`${service}/engine/inputs.sha256`, Buffer.from(manifest.join("")));

for (const [file, bytes] of output) {
  const path = join(root, file);
  if (check) {
    if (!bytes.equals(await readFile(path))) throw new Error(`Report asset is stale: ${file}`);
  } else {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, bytes);
  }
}
