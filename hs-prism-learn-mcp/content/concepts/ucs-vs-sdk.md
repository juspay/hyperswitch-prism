---
{
  "slug": "ucs-vs-sdk",
  "title": "UCS (this repo) vs the SDKs",
  "tier": "orientation",
  "audience": "everyone",
  "one_liner": "This repo is mostly the UCS Rust core (the engine); the SDKs are thin language wrappers your app actually imports.",
  "analogy": "An engine versus the steering wheel. UCS is the engine that does the work. The SDK is the steering wheel and pedals your app holds onto. Same car; different parts.",
  "depth": {
    "tldr": "UCS = the Rust core that does payments (this repo's crates/). SDK = the small Python/Node/Java/Rust library your app installs to call it. If you are contributing to connectors, you work in UCS. If you are building an app that takes payments, you use an SDK.",
    "standard": "UCS (Unified Connector Service) is the Rust core: connectors, transformers, the gRPC server, and the proto contract. The SDKs (under sdk/) are thin clients that call the core and expose ergonomic methods in each language. Knowing which one you are in saves confusion: a question like 'where is the Stripe request built?' is answered in UCS (crates/), not in an SDK. A question like 'how do I make my first charge from Node?' is answered by an SDK and by the integration docs.",
    "deep": "The SDKs bind to the core via FFI (and gRPC in server mode). For app-integration questions, this learn server hands off to the existing integration-mcp and the sdk-integration skill rather than duplicating them. This server is about understanding the repo itself."
  },
  "prerequisites": ["three-layer-architecture"],
  "related": ["what-is-prism", "repo-tour", "connector"],
  "go_deeper": [
    {"path": "docs/architecture/README.md", "why": "where the core ends and the SDKs begin"},
    {"path": ".skills/sdk-integration/SKILL.md", "why": "the skill for using the SDKs (app side)"}
  ],
  "verify_anchors": []
}
---
