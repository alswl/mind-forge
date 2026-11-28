import { useId } from "react"
import stamp from "./delivery.png"
import "./delivery.css"

export default function Metric({ value }: { value: number }) {
  const title = useId()
  return <dl className="delivery-metric" aria-labelledby={title}>
    <div>
      <dt id={title}><img src={stamp} width={12} height={12} alt="" /> Delivered</dt>
      <dd>{value} requests</dd>
    </div>
  </dl>
}
