export default function Weekly({ data, components }) {
  const Metric = components.get("repo/delivery")
  return <Metric value={data.completed} />
}
