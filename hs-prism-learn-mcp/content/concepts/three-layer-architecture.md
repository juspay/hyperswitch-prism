---
{
  "slug": "three-layer-architecture",
  "title": "The three layers: SDK, gRPC, Rust core",
  "tier": "orientation",
  "audience": "everyone",
  "one_liner": "Prism is built in three layers: thin language SDKs on top, a gRPC contract in the middle, and the Rust core (with all the connectors) at the bottom.",
  "analogy": "A restaurant. The menu in your language is the SDK. The waiter carrying a standard order ticket is the gRPC contract. The kitchen that actually cooks is the Rust core.",
  "depth": {
    "tldr": "Three layers, top to bottom: (1) SDKs you call from Python/Node/Java/Rust, (2) a gRPC contract that defines the messages, (3) the Rust core that does the real work and talks to processors. Your app touches the top; this repo is mostly the bottom.",
    "standard": "Layer 1 -- SDKs: small libraries in Python, Node, Java, and Rust that your application imports. Layer 2 -- the gRPC contract (payment.proto): the typed request/response schema every layer agrees on. Layer 3 -- the Rust core: the connectors and transformers that translate the unified request into each processor's API and back. The SDKs are thin; almost all logic lives in the Rust core. That is why, when you explore this repo, you spend most of your time in crates/.",
    "deep": "The SDKs reach the core through FFI bindings (and a gRPC server mode for microservice deployments). Because the contract is a single proto, all SDKs stay in sync automatically. When you add a connector you are adding to Layer 3; the proto (Layer 2) rarely changes; the SDKs (Layer 1) are generated. See ucs-vs-sdk, payment-proto, and connector."
  },
  "prerequisites": ["what-is-prism"],
  "related": ["ucs-vs-sdk", "payment-proto", "connector", "services-and-methods"],
  "go_deeper": [
    {"path": "docs/architecture/README.md", "why": "the layered diagram and component descriptions"}
  ],
  "verify_anchors": []
}
---
