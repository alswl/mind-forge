# Choose and verify a display

| Reader question | Display | Check |
| --- | --- | --- |
| How did a measure change over time? | Line | Time zone, period boundaries, units and gaps |
| How do categories compare? | Grouped bar | Same unit, comparable group definitions and zero baseline |
| How do two numeric measures vary together? | Scatter | Numeric axes, labels, units and no implied causation |
| How does a whole split across categories? | Stacked share | Common stated denominator, fractions from 0 to 1, no redistribution of missing values |
| What is the exact value? | Table | Scope, units, precision and missing label |

Author-ordered rows need unique dimension values. Store missing metrics as
`null`. For a zero denominator, set `denominator: 0` on the row and use null
shares. A partially reported row keeps its known fractions of the whole.
Mark commentary and scope in the same narrative for both views.

Registry extensions live next to `minds.yaml` under
`.mind-forge/report/components/SLUG` and `.mind-forge/report/renderers/SLUG`.
Use an exact `repo/SLUG` ID. For a reusable delivery value, register
`component.yaml` as `apiVersion: report.mind-forge/v1`, `kind: Component`,
`id: repo/delivery`, `version: 1.0.0`, `entry: Metric.tsx`,
`propsSchema: props.schema.json` and `example: example.tsx` with a short
description. Export a synchronous component from Metric.tsx and a synchronous
preview from example.tsx. The renderer declares the ID in `components` and
calls it with validated props.

```tsx
const Metric = components.get("repo/delivery")
return <Metric value={data.completed} />
```

Use a JSON Schema for props and a small synthetic example. The preview and
report must render on the server without browser lifecycle hooks. Keep
ordinary source hyperlinks and inline raster assets. External image loads,
CSS imports and scripts fail the offline export.
