---
{
  "slug": "flow",
  "title": "Flow: a single payment operation",
  "tier": "core-domain",
  "audience": "everyone",
  "one_liner": "A flow is one operation a connector can perform -- authorize, capture, refund, void, or a status sync. Connectors are built one flow at a time.",
  "analogy": "Buttons on a vending machine: 'reserve', 'pay', 'cancel', 'refund'. Each button is a flow. A connector decides which buttons it offers.",
  "depth": {
    "tldr": "A flow is one payment action. The six core ones are Authorize, Capture, Void, Refund, PSync (check payment status), and RSync (check refund status). You implement and test a connector flow by flow.",
    "standard": "Flows are the operations Prism exposes. Six are core: Authorize (reserve/charge), Capture (take reserved money), Void (cancel before capture), Refund (return money), PSync (poll a payment's status), RSync (poll a refund's status). More advanced flows exist too: webhooks, mandates (recurring), disputes. Flows have dependencies -- you cannot Capture without Authorize, and Refund depends on Capture -- so they are implemented in order. Each flow is a request/response pair the connector's transformer must map.",
    "deep": "Flow trait definitions live in crates/types-traits/domain_types/src/connector_flow.rs; per-flow implementation patterns live in .skills/_shared/references/flow-patterns/. The dependency graph is documented in flow-dependencies.md (note: it lists Refund's prerequisites as Authorize + Capture -- see the known-discrepancies, since one skill table lists only Authorize). See the individual flow-* cards."
  },
  "prerequisites": ["payments-101"],
  "related": ["flow-authorize", "flow-capture", "flow-refund", "flow-void", "flow-psync", "flow-rsync"],
  "go_deeper": [
    {"path": "docs/architecture/concepts/services-and-methods.md", "why": "how operations are grouped into services and methods"},
    {"path": ".skills/_shared/references/flow-implementation-guide.md", "why": "the three-part pattern for implementing any flow"}
  ],
  "verify_anchors": []
}
---
