---
{
  "slug": "flow-authorize",
  "title": "Authorize: the first step of every payment",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "Authorize asks the processor to reserve (or charge) the customer's money. It is the foundation -- every other flow builds on it.",
  "analogy": "A hotel placing a hold on your card at check-in. The money is reserved, not yet taken. Some shops take it immediately; that is automatic capture.",
  "depth": {
    "tldr": "Authorize is the first action: it reserves the customer's money (or charges it outright). Everything else -- capture, void, refund -- comes after an authorize.",
    "standard": "You send one unified authorize request; the connector's transformer rewrites it to the processor's format, sends it, and rewrites the reply back. With AUTOMATIC capture the money is taken right away (status CHARGED). With MANUAL capture it is only reserved (status AUTHORIZED) and you capture later. A decline is not an exception -- it comes back as a FAILURE status inside the response, so always check response.status.",
    "deep": "Authorize has no prerequisites; it is the root of the flow dependency graph. Implementation pattern is in flow-patterns/authorize.md; a real first call is in getting-started/first-payment.md. See flow-capture, flow-void, status-codes, and error-model."
  },
  "prerequisites": ["flow", "payments-101"],
  "related": ["flow-capture", "flow-void", "flow-psync", "status-codes", "error-model"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/authorize.md", "why": "the implementation pattern for Authorize"},
    {"path": "docs/getting-started/first-payment.md", "why": "a real first authorize call, end to end"}
  ],
  "verify_anchors": []
}
---
