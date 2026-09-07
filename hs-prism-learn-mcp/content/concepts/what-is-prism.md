---
{
  "slug": "what-is-prism",
  "title": "What hyperswitch-prism is",
  "tier": "payments-primer",
  "audience": "everyone",
  "one_liner": "Hyperswitch-prism is one library that lets you talk to 95+ payment processors through a single, unified request -- you write your payment code once, and it works with any of them.",
  "analogy": "A universal travel power adapter. Your charger (your payment request) never changes. The adapter reshapes the plug to fit each country's socket (each processor's API). Prism is that adapter, for payments.",
  "depth": {
    "tldr": "Prism is a tool that speaks to every payment processor for you. You send ONE kind of request; Prism rewrites it into whatever Stripe, Adyen, or PayPal expects, sends it, and gives you back ONE kind of answer. Write once, use any processor.",
    "standard": "Hyperswitch-prism (also called UCS -- the Unified Connector Service) is a stateless library that unifies 95+ payment processors behind a single request/response schema. You build a payment request in the unified format; Prism's per-processor 'connector' translates it to that processor's API and translates the response back to the unified format. It does not store card data or run a database -- it is a translation layer, not a payments gateway. It was extracted from Juspay Hyperswitch's hardened production integrations.",
    "deep": "The unified schema is defined in payment.proto. Each processor is a connector (a Rust struct plus transformers). The same library is wrapped by thin SDKs in Python, Node, Java, and Rust so apps in any language get the same behaviour. Because it is stateless and never stores card numbers, it keeps your PCI scope small. See three-layer-architecture, connector, and stateless."
  },
  "prerequisites": ["what-is-a-payment-processor"],
  "related": ["why-prism-exists", "unified-request", "ucs-vs-sdk", "stateless"],
  "go_deeper": [
    {"path": "README.md", "why": "the canonical project description and quick start"},
    {"path": "docs/architecture/README.md", "why": "the high-level architecture with diagrams"}
  ],
  "verify_anchors": [
    {"path": "README.md", "must_contain": "stateless"}
  ]
}
---
