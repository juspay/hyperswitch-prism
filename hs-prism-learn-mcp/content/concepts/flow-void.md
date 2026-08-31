---
{
  "slug": "flow-void",
  "title": "Void: cancel before you capture",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "Void cancels an authorization before it is captured, releasing the reserved funds. After capture you refund instead.",
  "analogy": "The hotel dropping the hold on your card because you cancelled before check-in. Nothing was charged, so nothing is refunded -- the reservation just disappears.",
  "depth": {
    "tldr": "Void cancels a payment that was authorized but not yet captured. It frees the held money. If the money was already captured, you cannot void -- you refund.",
    "standard": "Void undoes an Authorize that has not been captured. The connector builds the processor's cancel/void request from the authorized payment id and maps the result back; a successful void moves the payment to VOIDED. The distinction matters: void before capture, refund after capture.",
    "deep": "Void depends on Authorize. Pattern in flow-patterns/void.md. Some processors expose void as a cancel endpoint. See flow-authorize, flow-capture, and flow-refund."
  },
  "prerequisites": ["flow-authorize"],
  "related": ["flow-authorize", "flow-capture", "flow-refund", "status-codes"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/void.md", "why": "the implementation pattern for Void"}
  ],
  "verify_anchors": []
}
---
