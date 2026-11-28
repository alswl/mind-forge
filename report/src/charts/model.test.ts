import { describe, expect, it } from "vitest";
import { type ChartProps, metricText, presentation } from "./model";

const series = [{ key: "a", label: "Completed" }, { key: "b", label: "Remaining" }];
const props: ChartProps = { title: "Delivery", x: { key: "period", label: "Week" }, series, rows: [] };

describe("shared report presentation", () => {
  it("preserves zero, explicit missing data and small values", () => {
    const model = presentation({ ...props, rows: [
      { period: "Jan", a: 0, b: null }, { period: "Feb", a: 0.0000001, b: 7 },
    ] }, "line");
    expect(model.points.map(p => p.values.map(v => v.text))).toEqual([["0", "Not reported"], ["0.0000001", "7"]]);
    expect(metricText(0, "requests")).toBe("0 requests");
  });

  it("rejects absent records, inconsistent dimensions and bad dates with field context", () => {
    expect(() => presentation({ ...props, rows: [{ period: "Jan", a: 0 }] }, "bar")).toThrow("rows[0].b");
    expect(() => presentation({ ...props, rows: [
      { period: "Jan", a: 0, b: 1 }, { period: "Jan", a: 1, b: 0 },
    ] }, "bar")).toThrow("repeats");
    expect(() => presentation({ ...props, rows: [{ period: "2026-02-30", a: 0, b: 1 }],
      x: { key: "period", label: "Day", type: "date" },
    }, "line")).toThrow("not a valid date");
  });

  it("keeps known shares, uses the same denominator and marks zero denominators", () => {
    const stack = { ...props, denominator: "All requests in each period" };
    const model = presentation({ ...stack, rows: [
      { period: "Jan", a: 0.29, b: 0.71 }, { period: "Feb", a: 0.42, b: null },
      { period: "Mar", a: null, b: null, denominator: 0 },
    ] }, "stacked-share");
    expect(model.points.map(p => p.values.map(v => v.text))).toEqual([
      ["29%", "71%"], ["42%", "Not reported"], ["Not computable", "Not computable"],
    ]);
    expect(() => presentation({ ...stack, rows: [{ period: "Jan", a: 0.4, b: 0.4 }] }, "stacked-share")).toThrow("total 100%");
  });
});
