import type { ComponentType } from "react";
import { Chart } from "../charts/chart";
import type { ChartKind, ChartProps } from "../charts/model";
import { Document, Metric, Narrative } from "../components/document";
import type { RenderContext } from "../types";

interface Data {
  title: string;
  subtitle?: string;
  narrative?: string;
  metrics?: React.ComponentProps<typeof Metric>[];
  charts?: (ChartProps & { kind: ChartKind })[];
  rows?: ChartProps["rows"];
  x?: ChartProps["x"];
  series?: ChartProps["series"];
  rangeLabel?: string;
}

function report(kind?: ChartKind) {
  return ({ data: input, components }: RenderContext) => {
    const data = input as Data;
    const charts = [...data.charts ?? []];
    if (kind && data.rows && data.x && data.series) {
      charts.unshift({ kind, title: kind === "line" ? "Change over time" : "Category comparison",
        rows: data.rows, x: data.x, series: data.series, rangeLabel: data.rangeLabel });
    }
    const renderChart = (chart: ChartProps & { kind: ChartKind }, i: number) => {
      const Display = components.get("builtin/" + chart.kind);
      return <Display key={i} {...chart} />;
    };
    const MetricCard = data.metrics?.length ? components.get("builtin/metric") : undefined;
    return <Document title={data.title} subtitle={data.subtitle}>
      {data.narrative && <Narrative text={data.narrative} />}
      {MetricCard && <section className="metrics" aria-label="Metrics">
        {data.metrics?.map((metric, i) => <MetricCard key={i} {...metric} />)}
      </section>}
      {charts.map(renderChart)}
    </Document>;
  };
}

export const renderers = {
  brief: report(),
  comparison: report("bar"),
  trend: report("line"),
} satisfies Record<string, ComponentType<RenderContext>>;

const display = (kind: ChartKind) => (props: ChartProps & Pick<RenderContext, "view">) => <Chart {...props} kind={kind} />;

export const components = {
  metric: Metric,
  line: display("line"),
  bar: display("bar"),
  scatter: display("scatter"),
  "stacked-share": display("stacked-share"),
  table: display("table"),
  document: Document,
  narrative: Narrative,
};

const series = [{ key: "current", label: "Current" }, { key: "previous", label: "Previous" }];
const category = { key: "category", label: "Group" };
const sequence = [
  { category: "Spring", current: 0, previous: 5 },
  { category: "Summer", current: null, previous: 6 },
  { category: "Autumn", current: 7, previous: 4 },
];
export const examples = {
  metric: { label: "Open requests", value: 0, unit: "requests", note: "At period end" },
  document: { title: "Delivery notes", subtitle: "January to March", children: "An evidence summary." },
  narrative: { title: "What changed", text: "The quieter period let the team clear its backlog." },
  table: { title: "Requests by period", x: category, series, rows: sequence },
  bar: { title: "Requests by period", x: category, series, rows: sequence },
  line: { title: "Requests by date", x: { key: "date", label: "Week starting", type: "date" }, series,
    rows: sequence.map((row, i) => ({ ...row, date: `2026-03-${String(i * 7 + 1).padStart(2, "0")}` })) },
  scatter: { title: "Requests by exposure", x: { key: "hours", label: "Exposure", type: "number", unit: "h" }, series,
    rows: sequence.map((row, i) => ({ ...row, hours: i * 2.5 })) },
  "stacked-share": { title: "Share by period", x: category, series, denominator: "Share of all requests in each period",
    rows: [{ category: "Spring", current: 0.28, previous: 0.72 },
      { category: "Summer", denominator: 0, current: null, previous: null },
      { category: "Autumn", current: null, previous: 0.42 }] },
};
