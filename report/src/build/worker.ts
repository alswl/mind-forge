import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ComponentType } from "react";
import Ajv from "ajv";
import Ajv2019 from "ajv/dist/2019";
import Ajv2020 from "ajv/dist/2020";
import { compile, Unavailable } from "./compile";
import { page } from "./document";
import type { Job, Module } from "../types";

const protocol = "report.mind-forge/v1";

function validator(definition: Module) {
  const schema = definition.schema as { $schema?: string };
  const dialect = schema?.$schema ?? "";
  const options = { strict: false, allErrors: true, validateFormats: false };
  const engine = dialect.includes("draft-07") || dialect.includes("draft-06") ? new Ajv(options)
    : dialect.includes("2019-09") ? new Ajv2019(options) : new Ajv2020(options);
  if (dialect.includes("draft-06")) engine.addMetaSchema(require("ajv/dist/refs/json-schema-draft-06.json"));
  const validate = engine.compile(definition.schema);
  return (value: unknown, label: string) => {
    if (!validate(value)) {
      throw new Error(`${label}: ${validate.errors?.map(error => `${error.instancePath || "/"} ${error.message}`).join("; ")}`);
    }
  };
}

function syncComponent(value: unknown, label: string) {
  if (typeof value !== "function" || value.constructor.name === "AsyncFunction") {
    throw new Error(`${label} must default export a synchronous React component`);
  }
  return value as ComponentType<Record<string, unknown>>;
}

export async function render(job: Job, embedded: string) {
  if (job.protocol !== protocol) throw new Error(`unsupported protocol ${String(job.protocol)}`);
  const scratch = await mkdtemp(join(tmpdir(), "mf-report-render-"));
  try {
    const { code, react, server, css } = await compile(job, scratch, embedded);
    const el = react.createElement;
    if (job.catalog) {
      const entries = await Promise.all(job.catalog.map(async (item, index) => {
        const Preview = syncComponent(code.components.get(item.id), item.id);
        const key = `example-${index}`;
        const builtin = item.id.startsWith("builtin/");
        const props = code.examples[item.id.replace(/^builtin\//, "")];
        const codeText = builtin ? `const Preview = components.get(${JSON.stringify(item.id)})\n<Preview {...${JSON.stringify(props, null, 2)}} />`
          : await readFile(item.example, "utf8");
        if (builtin) validator(item)(props, item.id);
        return el("section", { className: "catalog-entry", key: item.id },
          el("h2", null, item.id), el("p", { className: "report-note" }, item.description || item.version),
          el(Preview, builtin ? { ...props, view: "chart", theme: job.theme } : { view: "chart", theme: job.theme }),
          el("details", { className: "catalog-code" }, el("summary", null, "Example"),
            el("pre", { id: key }, el("code", null, codeText)),
            el("button", { type: "button", hidden: true, "data-copy": key }, "Copy template"),
            el("span", { role: "status" })),
          el("details", null, el("summary", null, "Props schema"),
            el("pre", null, el("code", null, JSON.stringify(item.schema, null, 2)))));
      }));
      return page("Report component catalog", server.renderToStaticMarkup(el("article", null,
        el("header", { className: "report-heading" }, el("h1", null, "Report component catalog")),
        entries)), css, job.theme, true);
    }
    const data = JSON.parse(await readFile(job.data, "utf8"));
    validator(job.renderer)(data, `renderer ${job.renderer.id} data`);
    const available = new Map(job.components.map(module => {
      const Component = syncComponent(code.components.get(module.id), module.id);
      const verify = validator(module);
      return [module.id, (props: Record<string, unknown>) => {
        verify(props, `renderer ${job.renderer.id} component ${module.id} props`);
        return el(Component, { ...props, view: job.view, theme: job.theme });
      }];
    }));
    const components = { get(id: string) {
      const Component = available.get(id);
      if (!Component) throw new Error(`renderer ${job.renderer.id} did not declare component ${id}`);
      return Component;
    } };
    const Report = syncComponent(code.Report, "report entry");
    const Renderer = syncComponent(code.Renderer, job.renderer.id);
    const context = { Renderer, data, view: job.view, theme: job.theme, components };
    const title = typeof data?.title === "string" ? data.title : "Report";
    return page(title, server.renderToStaticMarkup(el(Report, context)), css, job.theme, job.view === "chart");
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

async function main() {
  const receipt = process.stdout.write.bind(process.stdout);
  // Authored console output belongs to diagnostics, leaving stdout for the receipt.
  process.stdout.write = process.stderr.write.bind(process.stderr);
  try {
    if (process.versions.node.split(".")[0] !== "24") {
      throw new Unavailable(`Node 24 is required, found Node ${process.versions.node}`);
    }
    const input: Buffer[] = [];
    for await (const piece of process.stdin) input.push(piece);
    const job = JSON.parse(Buffer.concat(input).toString("utf8")) as Job;
    const html = await render(job, join(__dirname, "builtins.cjs"));
    await writeFile(job.stage, html, { flag: "r+" });
    receipt(JSON.stringify({ protocol, stage: job.stage, result: {
      protocol, renderer: job.renderer.id, rendererVersion: job.renderer.version, view: job.view, artifact: job.artifact,
    } }) + "\n");
  } catch (error) {
    process.stderr.write(`report engine: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = error instanceof Unavailable ? 3 : 2;
  }
}

if (require.main === module) void main();
