import type { ReactNode } from "react";
import { metricText } from "../charts/model";

export function Document({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  return <article><header className="report-heading">
    <h1>{title}</h1>{subtitle && <p>{subtitle}</p>}
  </header>{children}</article>;
}

export function Narrative({ title, text }: { title?: string; text: string }) {
  return <section className="narrative">{title && <h2>{title}</h2>}
    {text.split(/\n\n+/).map((paragraph, index) => <p key={index}>{paragraph}</p>)}
  </section>;
}

export function Metric({ label, value, unit, note, missingLabel }: {
  label: string; value: number | null; unit?: string; note?: string; missingLabel?: string;
}) {
  return <dl className="metric"><div>
    <dt>{label}</dt><dd>{metricText(value, unit, missingLabel)}</dd>
    {note && <dd className="metric-note">{note}</dd>}
  </div></dl>;
}
