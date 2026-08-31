---
{
  "slug": "flow-capture",
  "title": "Capture: take the money you reserved",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "Capture takes money that an earlier Authorize reserved. You use it when you reserve first and charge later (manual capture).",
  "analogy": "The hotel charging your final bill at check-out, against the hold they placed at check-in. Capture is the check-out charge.",
  "depth": {
    "tldr": "Capture actually moves the money that Authorize reserved. It only makes sense after an Authorize that did not already charge (i.e. manual capture).",
    "standard": "When a payment is authorized with MANUAL capture, the funds are reserved but not taken. Capture is the flow that takes them -- in full or partially. The connector's transformer builds the processor's capture request from the authorized payment's id, sends it, and maps the result back. A successful capture moves the payment to CHARGED.",
    "deep": "Capture depends on Authorize (you must authorize before you can capture). Pattern in flow-patterns/capture.md. Partial capture and multiple captures are processor-dependent. See flow-authorize, flow-void, and flow-refund."
  },
  "prerequisites": ["flow-authorize"],
  "related": ["flow-authorize", "flow-void", "flow-refund", "status-codes"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/capture.md", "why": "the implementation pattern for Capture"},
    {"path": "docs/getting-started/extend-to-more-flows.md", "why": "adding capture/void/refund after your first payment"}
  ],
  "verify_anchors": []
}
---
