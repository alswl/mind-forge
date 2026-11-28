export interface Series {
  key: string;
  label: string;
  unit?: string;
}

export interface Dimension extends Series {
  type?: "category" | "number" | "date";
  timeZone?: string;
}

export interface ChartProps {
  title: string;
  rows: Record<string, unknown>[];
  x: Dimension;
  series: Series[];
  rangeLabel?: string;
  denominator?: string;
  missingLabel?: string;
}

export type ChartKind = "line" | "bar" | "scatter" | "stacked-share" | "table";

export const numberText = (value: number): string => {
  const exact = String(value);
  if (!exact.includes("e")) return exact;
  // Show the exact JS number without rounding small rates to zero.
  const [mantissa, exponent] = exact.split("e");
  const sign = mantissa.startsWith("-") ? "-" : "";
  const digits = mantissa.replace(/[-.]/g, "");
  const point = mantissa.replace("-", "").split(".")[0].length + Number(exponent);
  if (Math.abs(point) > 18) return exact;
  if (point <= 0) return sign + "0." + "0".repeat(-point) + digits;
  if (point >= digits.length) return sign + digits + "0".repeat(point - digits.length);
  return sign + digits.slice(0, point) + "." + digits.slice(point);
};

export function metricText(value: number | null, unit?: string, missing = "Not reported"): string {
  if (value === null) return missing;
  if (!Number.isFinite(value)) throw new Error("expected a finite value");
  return numberText(value) + (unit ? " " + unit : "");
}

function shareText(value: number): string {
  // Shift the decimal rather than multiplying a binary float by 100 for display.
  const [mantissa, exponent = "0"] = String(value).split("e");
  return numberText(Number(mantissa + "e" + (Number(exponent) + 2))) + "%";
}

export function presentation(input: ChartProps, kind: ChartKind) {
  const { rows, x, series, missingLabel = "Not reported", denominator } = input;
  if (!Array.isArray(rows) || !series?.length || !x?.key || !x.label) {
    throw new Error("chart requires rows, a dimension and at least one series");
  }
  if (new Set(series.map(s => s.key)).size !== series.length) {
    throw new Error("series keys must be unique");
  }
  if (kind === "stacked-share" && !denominator?.trim()) {
    throw new Error("stacked shares require a denominator description");
  }
  if (kind === "scatter" && x.type !== "number") {
    throw new Error("scatter requires a numeric x dimension");
  }
  const distinct = new Set<string | number>();
  const points = rows.map((row, index) => {
    const dimension = row[x.key];
    let position: string | number;
    let label: string;
    switch (x.type) {
      case "number":
        if (typeof dimension !== "number" || !Number.isFinite(dimension)) {
          throw new Error(`rows[${index}].${x.key} must be a finite number`);
        }
        position = dimension;
        label = metricText(dimension, x.unit);
        break;
      case "date": {
        if (typeof dimension !== "string" || !/^\d{4}-\d{2}-\d{2}(?:T.*(?:Z|[+-]\d{2}:\d{2}))?$/.test(dimension)) {
          throw new Error(`rows[${index}].${x.key} must be an ISO date or timestamp with an offset`);
        }
        position = Date.parse(dimension);
        if (!Number.isFinite(position) || (dimension.length === 10 && new Date(position).toISOString().slice(0, 10) !== dimension)) {
          throw new Error(`rows[${index}].${x.key} is not a valid date`);
        }
        label = dimension.length === 10 ? dimension : new Intl.DateTimeFormat("en-CA", {
          timeZone: x.timeZone ?? "UTC", dateStyle: "medium", timeStyle: "short",
        }).format(position);
        break;
      }
      default:
        if (typeof dimension !== "string" || !dimension.trim()) {
          throw new Error(`rows[${index}].${x.key} must be a nonempty label`);
        }
        position = label = dimension;
    }
    if (distinct.has(position)) throw new Error(`rows[${index}].${x.key} repeats dimension ${label}`);
    distinct.add(position);
    const values = series.map(({ key, unit }) => {
      const value = row[key];
      if (value !== null && (typeof value !== "number" || !Number.isFinite(value))) {
        throw new Error(`rows[${index}].${key} must be a finite number or explicit null`);
      }
      if (kind === "stacked-share" && value !== null && (value < 0 || value > 1)) {
        throw new Error(`rows[${index}].${key} must be a share between 0 and 1`);
      }
      return { value, text: kind === "stacked-share" && value !== null
        ? shareText(value)
        : metricText(value, unit, kind === "stacked-share" && row.denominator === 0 ? "Not computable" : missingLabel) };
    });
    if (kind === "stacked-share") {
      if (row.denominator === 0 && values.some(v => v.value !== null)) {
        throw new Error(`rows[${index}] has a zero denominator and must use null shares`);
      }
      const total = values.reduce((sum, v) => sum + (v.value ?? 0), 0);
      if (total > 1 + 1e-10 || (values.every(v => v.value !== null) && Math.abs(total - 1) > 1e-10)) {
        throw new Error(`rows[${index}] shares must use the documented denominator and total 100% when complete`);
      }
    }
    return { label, position, values };
  });
  return { ...input, points, kind };
}

export type Presentation = ReturnType<typeof presentation>;
