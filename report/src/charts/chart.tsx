import { scaleBand, scaleLinear, scalePoint } from "d3-scale";
import { line } from "d3-shape";
import { useId } from "react";
import { type ChartKind, type ChartProps, type Presentation, metricText, presentation } from "./model";
import type { View } from "../types";
import { plotLayout, tickIndices, yLabelGap } from "./layout";

const height = 360;
const top = 20;
const bottom = 298;

export function Chart(props: ChartProps & { kind: ChartKind; view: View }) {
  const model = presentation(props, props.kind);
  return <section className="report-block">
    <h2>{model.title}</h2>
    {(model.rangeLabel || model.denominator) && <p className="report-note">
      {[model.rangeLabel, model.denominator].filter(Boolean).join(" / ")}
    </p>}
    {model.points.length === 0 ? <p>No observations.</p> : props.view === "table" || props.kind === "table"
      ? <DataTable model={model} /> : <Plot model={model} />}
  </section>;
}

function DataTable({ model }: { model: Presentation }) {
  return <div className="table-scroll" tabIndex={0} role="region" aria-label={model.title}>
    <table>
      <caption className="visually-hidden">{model.title}</caption>
      <thead><tr><th scope="col">{model.x.label}</th>{model.series.map(s =>
        <th scope="col" key={s.key}>{s.label}{s.unit && <small>{s.unit}</small>}</th>)}</tr></thead>
      <tbody>{model.points.map(point => <tr key={point.position}>
        <th scope="row">{point.label}</th>
        {point.values.map((value, i) => <td key={model.series[i].key}>{value.text}</td>)}
      </tr>)}</tbody>
    </table>
  </div>;
}

function Plot({ model }: { model: Presentation }) {
  const title = useId();
  const { points, series, kind } = model;
  const stacked = kind === "stacked-share";
  const bars = kind === "bar" || stacked;
  const continuous = !bars && (model.x.type === "number" || model.x.type === "date");
  const values = points.flatMap(p => p.values.flatMap(v => v.value === null ? [] : [v.value]));
  const min = stacked ? 0 : values.reduce((a, b) => Math.min(a, b), 0);
  const max = stacked ? 1 : values.reduce((a, b) => Math.max(a, b), 0);
  const y = scaleLinear().domain([min, max === min ? max + 1 : max]).range([bottom, top]).nice();
  const unit = new Set(series.map(s => s.unit ?? "")).size === 1 ? series[0].unit : undefined;
  const yTicks = y.ticks(5).map(value => ({
    value,
    label: stacked ? `${Math.round(value * 100)}%` : metricText(Number(value.toPrecision(8)), unit),
  }));
  const xTicks = tickIndices(points.length).map(index => points[index]);
  const { width, left, right, minWidthRem } = plotLayout(
    yTicks.map(tick => tick.label), xTicks.map(tick => tick.label), model.x.label,
  );
  const positions = points.map(p => p.position);
  const band = scaleBand<string | number>().domain(positions).range([left, right]).padding(0.24);
  const pointScale = scalePoint<string | number>().domain(positions).range([left, right]).padding(0.3);
  const xNums = positions.map(Number);
  const xMin = xNums.reduce((a, b) => Math.min(a, b), Infinity);
  const xMax = xNums.reduce((a, b) => Math.max(a, b), -Infinity);
  const linear = scaleLinear().domain([xMin, xMax]).range([left + 9, right - 9]);
  const x = (value: string | number) => bars ? band(value)! + band.bandwidth() / 2
    : continuous ? linear(Number(value)) : pointScale(value)!;
  const group = scaleBand<number>().domain(series.map((_, i) => i)).range([0, band.bandwidth()]).padding(0.12);
  return <>
    <ul className="plot-key" aria-label="Series">{series.map((s, i) => <li key={s.key}>
      <span className="series-swatch" data-series={i % 6} aria-hidden="true" />
      {s.label}{s.unit ? ` (${s.unit})` : ""}
    </li>)}</ul>
    <div className="plot-scroll" tabIndex={0} role="region" aria-label={model.title}>
      <svg viewBox={`0 0 ${width} ${height}`} style={{ minWidth: `${minWidthRem}rem` }} role="group" aria-labelledby={title}>
        <title id={title}>{model.title}</title>
        {yTicks.map(({ value, label }) => <g key={value} className={value === 0 ? "plot-baseline" : "plot-grid"}>
          <line x1={left} x2={right} y1={y(value)} y2={y(value)} />
          <text x={left - yLabelGap} y={y(value)} dy=".32em" textAnchor="end">{label}</text>
        </g>)}
        {xTicks.map(p =>
          <text className="axis-tick" key={p.position} x={x(p.position)} y={bottom + 23} textAnchor="middle">{p.label}</text>)}
        <text className="axis-title" x={(left + right) / 2} y={height - 10} textAnchor="middle">{model.x.label}</text>
        {kind === "line" && series.map((s, i) => <path key={s.key} aria-hidden="true" className="plot-line"
          data-series={i % 6} d={line<(typeof points)[number]>()
            .defined(p => p.values[i].value !== null)
            .x(p => x(p.position)).y(p => y(p.values[i].value!))(points) ?? ""} />)}
        {points.map(point => {
          let share = 0;
          return <g key={point.position}>{point.values.map(({ value, text }, i) => {
            const label = `${point.label} / ${series[i].label}: ${text}`;
            const attrs = { tabIndex: 0, role: "img", "aria-label": label, "data-tooltip": label, "data-series": i % 6 };
            if (value === null) {
              const pos = bars && !stacked ? band(point.position)! + group(i)! + group.bandwidth() / 2 : x(point.position);
              return <text key={series[i].key} {...attrs} className="plot-missing" x={pos + (stacked ? (i - (series.length - 1) / 2) * 13 : 0)}
                y={bottom + 5} textAnchor="middle"><title>{label}</title>×</text>;
            }
            if (!bars) return <circle key={series[i].key} {...attrs} cx={x(point.position)} cy={y(value)} r={5}><title>{label}</title></circle>;
            const start = stacked ? share : 0;
            if (stacked) share += value;
            const h = Math.abs(y(start) - y(start + value));
            return <rect key={series[i].key} {...attrs} x={band(point.position)! + (stacked ? 0 : group(i)!)}
              y={Math.min(y(start), y(start + value)) - (h < 2 ? 1 : 0)} width={stacked ? band.bandwidth() : group.bandwidth()}
              height={Math.max(h, 2)}><title>{label}</title></rect>;
          })}</g>;
        })}
      </svg>
    </div>
  </>;
}
