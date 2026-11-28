import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { expect, it } from "vitest";
import { render } from "./worker";
import type { Job } from "../types";

it("renders placed component entries without loading the full-page entry", async () => {
  const workDir = process.cwd();
  const temp = await mkdtemp(join(workDir, "fragment-test-"));
  try {
    const entry = temp.slice(workDir.length + 1) + "/panels/metric.tsx";
    await mkdir(join(temp, "panels"));
    await writeFile(join(workDir, entry), "export default ({ data, width }) => <text x={width}>{data.value}</text>");
    const embedded = join(temp, "builtins.cjs");
    await writeFile(embedded, "exports.renderers = { brief: () => null };");
    const data = join(temp, "data.json");
    await writeFile(data, '{"value":7}');
    const job: Job = {
      protocol: "report.mind-forge/v1", workDir, entry: "unused.tsx", data,
      renderer: { id: "builtin/brief", entry: "", version: "1.0.0", schema: true },
      components: [], view: "fragment", theme: "system", stage: "", artifact: "overview.svg",
      fragment: { width: 100, height: 40, components: [
        { entry, x: 0, y: 0, width: 50, height: 40 },
        { entry, x: 50, y: 0, width: 40, height: 30 },
      ] },
    };
    const svg = await render(job, embedded);
    expect(svg).toMatch(/^<svg /);
    expect(svg).toContain('<text x="50">7</text>');
    expect(svg).toContain('<svg x="50" y="0" width="40" height="30"');
    expect(svg).toContain('<text x="40">7</text>');
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
