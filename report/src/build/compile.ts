import { createRequire } from "node:module";
import { writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, sep } from "node:path";
import type { BuildOptions } from "esbuild";
import type { Job, Module } from "../types";

export class Unavailable extends Error {}

const quoted = JSON.stringify;
const builtin = (module: Module) => module.id.startsWith("builtin/");
const member = (id: string) => quoted(id.slice("builtin/".length));
const resolver = /^(react(-dom)?)(\/.*)?$/;

export async function compile(job: Job, scratch: string, embedded: string) {
  const requireReport = createRequire(join(job.workDir, "package.json"));
  let esbuild: typeof import("esbuild");
  let react: typeof import("react");
  let server: typeof import("react-dom/server");
  try {
    for (const dependency of ["esbuild", "react", "react-dom/server"]) {
      const location = relative(join(job.workDir, "node_modules"), requireReport.resolve(dependency));
      if (isAbsolute(location) || location === ".." || location.startsWith(".." + sep)) {
        throw new Error(`${dependency} must resolve from the report workdir's node_modules`);
      }
    }
    esbuild = requireReport("esbuild");
    react = requireReport("react");
    server = requireReport("react-dom/server");
    const rootReact = requireReport.resolve("react");
    const domReact = createRequire(requireReport.resolve("react-dom")).resolve("react");
    if (domReact !== rootReact) throw new Error("react-dom resolves a different React installation");
    if (!react.version.startsWith("19.") || server.version !== react.version) throw new Error("React and React DOM must use the same 19.x version");
  } catch (error) {
    throw new Unavailable(`report dependencies are unavailable in ${job.workDir}. Run pnpm install --frozen-lockfile there. ${String(error)}`);
  }
  const sources = [`import * as builtins from ${quoted(embedded)};`];
  const specs = job.catalog ?? job.components;
  for (const [index, module] of specs.entries()) {
    if (!builtin(module)) {
      sources.push(`import C${index} from ${quoted(job.catalog ? job.catalog[index].example : module.entry)};`);
    }
  }
  const entries = specs.map((module, index) => {
    let value = `C${index}`;
    if (builtin(module)) value = `builtins.components[${member(module.id)}]`;
    return `[${quoted(module.id)}, ${value}]`;
  });
  if (job.catalog) {
    sources.push("export const examples = builtins.examples;");
  } else {
    if (builtin(job.renderer)) {
      sources.push(`export const Renderer = builtins.renderers[${member(job.renderer.id)}];`);
    } else {
      sources.push(`export {default as Renderer} from ${quoted(job.renderer.entry)};`);
    }
    if (!job.fragment) sources.push(`export {default as Report} from ${quoted(job.entry)};`);
  }
  sources.push(`export const components = new Map([${entries.join(",")}]);`);
  if (job.fragment) {
    const names = job.fragment.components.map(({entry}, i) => {
      sources.push(`import Panel${i} from ${quoted(join(job.workDir, entry))};`);
      return `Panel${i}`;
    });
    sources.push(`export const panels = [${names.join(",")}];`);
  }
  const moduleFile = join(scratch, "render.cjs");
  const options = {
    stdin: { contents: sources.join("\n"), resolveDir: job.workDir, sourcefile: "report-entry.ts" },
    absWorkingDir: job.workDir,
    bundle: true,
    platform: "node",
    target: "node24",
    format: "cjs",
    outfile: moduleFile,
    jsx: "automatic",
    logLevel: "silent",
    nodePaths: [join(job.workDir, "node_modules")],
    write: false,
    loader: { ".png": "dataurl", ".jpg": "dataurl", ".jpeg": "dataurl", ".webp": "dataurl", ".gif": "dataurl", ".avif": "dataurl" },
    plugins: [{
      name: "report-react",
      setup(build) {
        build.onResolve({ filter: resolver }, ({ path }) => ({
          path: requireReport.resolve(path),
          external: true,
        }));
        // A report can install its own dependencies. Avoid choosing an
        // extension's unrelated ancestor package when it names a bare import.
        build.onResolve({ filter: /^[^./#][^:]*$/ }, async (args) => {
          if (!args.importer || args.pluginData === "report-workdir") return;
          const workdir = await build.resolve(args.path, {
            kind: args.kind, resolveDir: job.workDir, importer: args.importer, pluginData: "report-workdir",
          });
          if (!workdir.errors.length) return workdir;
          if (dirname(args.importer).startsWith(join(job.workDir, "node_modules"))) return;
          return { errors: [{ text: `cannot resolve ${args.path} from the report workdir` }] };
        });
      },
    }],
  } satisfies BuildOptions;
  const built = await esbuild.build(options);
  let css = "";
  for (const output of built.outputFiles) {
    if (output.path === moduleFile) await writeFile(moduleFile, output.contents, { mode: 0o600 });
    else if (output.path.endsWith(".css")) css += output.text;
    else throw new Error(`unsupported bundled asset ${output.path}`);
  }
  return { code: requireReport(moduleFile), react, server, css };
}
