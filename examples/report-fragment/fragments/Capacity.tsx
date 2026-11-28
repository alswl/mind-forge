import { scaleLinear } from "d3-scale";

export default function Capacity({ data, width }) {
  const x = scaleLinear().domain([0, data.capacity]).range([4, width - 4]);
  return <g>
    <defs><linearGradient id="ink"><stop stopColor="#35499c" /></linearGradient></defs>
    <text x={4} y={24}>Capacity: {data.capacity}</text>
    <line x1={x(0)} x2={x(data.capacity)} y1={48} y2={48} stroke="#adb3be" />
    <circle cx={x(data.completed)} cy={48} r={5} fill="url(#ink)" />
  </g>;
}
