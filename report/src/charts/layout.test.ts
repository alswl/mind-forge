import { describe, expect, it } from "vitest";
import { plotLayout, tickIndices, yLabelGap } from "./layout";

describe("static chart axis layout", () => {
  it("keeps the first and last observations when thinning ticks", () => {
    expect(tickIndices(0)).toEqual([]);
    expect(tickIndices(1)).toEqual([0]);
    expect(tickIndices(3)).toEqual([0, 1, 2]);
    expect(tickIndices(14)).toEqual([0, 2, 4, 7, 9, 11, 13]);
  });

  it("reserves space on both sides of the plot for centered dates", () => {
    const { width, left, right } = plotLayout(["0 requests", "8 requests"], ["2026-01-05", "2026-01-19"], "Week starting");
    expect(left - yLabelGap).toBeGreaterThan(75);
    expect(left).toBeGreaterThan(40);
    expect(width - right).toBeGreaterThan(40);
  });

  it("widens the chart with long units and keeps room for the plot and readable text", () => {
    const dates = ["2026-01-05", "2026-01-19"];
    const normal = plotLayout(["0 requests", "8 requests"], dates, "Week starting");
    const wide = plotLayout(["0 requests per reporting period", "8 requests per reporting period"], dates, "Week starting");
    expect(wide.left).toBeGreaterThan(normal.left);
    expect(wide.right - wide.left).toBeGreaterThanOrEqual(460);
    expect(wide.minWidthRem / wide.width).toBeCloseTo(normal.minWidthRem / normal.width);
  });
});
