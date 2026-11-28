import { Rectangle } from "recharts";

export default function Completed({ data, width }) {
  return <g>
    <defs><linearGradient id="ink"><stop stopColor="#137272" /></linearGradient></defs>
    <text x={4} y={24}>Completed: {data.completed}</text>
    <Rectangle x={4} y={36} width={(width - 8) * data.completed / data.capacity}
      height={22} radius={[3, 3, 3, 3]} fill="url(#ink)" />
  </g>;
}
