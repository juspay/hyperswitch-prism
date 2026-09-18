---
{
  "slug": "flow-refund",
  "title": "Refund: give captured money back",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "Refund returns money after it has been captured -- in full or partially. It depends on Capture (you can only refund money you actually took).",
  "analogy": "The shop returning money to your card after you have already paid and walked out. The sale happened; now part or all of it is reversed.",
  "depth": {
    "tldr": "Refund sends captured money back to the customer. Because you can only return money you actually took, Refund comes after Capture.",
    "standard": "Refund reverses a captured payment, fully or partially. The connector builds the processor's refund request from the captured payment id and amount, sends it, and maps the result back. Refunds are often asynchronous -- the processor accepts the request and finalizes later -- which is why there is a separate RSync flow to poll refund status.",
    "deep": "Refund's prerequisites are Authorize and Capture per flow-dependencies.md (note: the add-connector-flow SKILL table lists only Authorize -- a real discrepancy this server surfaces; the dependency-graph reference is authoritative). Pattern in flow-patterns/refund.md. See flow-rsync and the known-discrepancies."
  },
  "prerequisites": ["flow-capture"],
  "related": ["flow-capture", "flow-rsync", "status-codes"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/refund.md", "why": "the implementation pattern for Refund"},
    {"path": ".skills/add-connector-flow/references/flow-dependencies.md", "why": "the authoritative flow dependency graph (Refund needs Authorize + Capture)"}
  ],
  "verify_anchors": [
    {"path": ".skills/add-connector-flow/references/flow-dependencies.md", "must_contain": "Refund"}
  ]
}
---
