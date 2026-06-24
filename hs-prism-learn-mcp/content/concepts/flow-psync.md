---
{
  "slug": "flow-psync",
  "title": "PSync: ask the processor for the payment's status",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "PSync (Payment Sync) asks the processor for the current status of a payment, used when the result is not known immediately or arrives later.",
  "analogy": "Tracking a parcel. You placed the order (authorize); now you check 'where is it?' PSync is the tracking query for a payment.",
  "depth": {
    "tldr": "PSync checks a payment's current status by asking the processor again. You use it when a payment is pending or when you need to confirm the latest state.",
    "standard": "Some payments do not finish instantly -- the customer may need to complete a bank redirect, or the processor may confirm later. PSync polls the processor for the payment's current status and maps it back to Prism's status codes. It is read-only: it changes nothing, it just reports.",
    "deep": "PSync depends on Authorize. It is also a prerequisite for incoming webhooks in the dependency graph (webhooks reuse the status-mapping PSync establishes). Pattern in flow-patterns/psync.md. See flow-authorize and status-codes."
  },
  "prerequisites": ["flow-authorize"],
  "related": ["flow-authorize", "flow-rsync", "status-codes"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/psync.md", "why": "the implementation pattern for PSync"}
  ],
  "verify_anchors": []
}
---
