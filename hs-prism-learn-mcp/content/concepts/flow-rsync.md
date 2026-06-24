---
{
  "slug": "flow-rsync",
  "title": "RSync: ask the processor for the refund's status",
  "tier": "core-flow",
  "audience": "everyone",
  "one_liner": "RSync (Refund Sync) asks the processor for the current status of a refund, because refunds often finalize asynchronously.",
  "analogy": "Checking whether a returned-item refund has landed back on your card. You requested it; RSync asks 'is it done yet?'",
  "depth": {
    "tldr": "RSync checks a refund's current status. Refunds frequently complete later, so you poll with RSync to find out when they succeed.",
    "standard": "When you request a Refund, many processors accept it as pending and finalize it later. RSync polls the processor for the refund's current status and maps it back to Prism's refund status codes. Like PSync, it is read-only and reports state without changing anything.",
    "deep": "RSync depends on Refund. Pattern in flow-patterns/rsync.md. See flow-refund and status-codes."
  },
  "prerequisites": ["flow-refund"],
  "related": ["flow-refund", "flow-psync", "status-codes"],
  "go_deeper": [
    {"path": ".skills/_shared/references/flow-patterns/rsync.md", "why": "the implementation pattern for RSync"}
  ],
  "verify_anchors": []
}
---
