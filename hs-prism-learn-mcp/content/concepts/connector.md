---
{
  "slug": "connector",
  "title": "Connector: one adapter per payment processor",
  "tier": "core-domain",
  "audience": "everyone",
  "one_liner": "A connector is the small piece of code that teaches Prism how to talk to ONE processor. There are ~95, and they all share the same shape.",
  "analogy": "A travel power-plug adapter. Your charger (the unified request) never changes; the adapter reshapes the plug to fit each country's socket (the processor's API). A connector is that adapter, for one processor.",
  "depth": {
    "tldr": "A connector is the code for one processor (Stripe, Adyen, ...). It knows that processor's API. All ~95 connectors look alike, so once you have read one you can read any of them.",
    "standard": "A connector is a Rust struct plus a transformers.rs file. The struct wires up the flows the processor supports (authorize, capture, refund, ...); the transformers do the reshaping -- turning Prism's unified request into the processor's exact fields, and the reply back into the unified response. Connectors are stateless and generic over the payment-method type T, which is why one connector can handle cards, wallets, and bank transfers. Every connector is registered in connectors.rs and lives next to the others in the connectors/ folder.",
    "deep": "Look at Stripe as the canonical example: stripe.rs declares the struct and uses macros (create_all_prerequisites! and macro_connector_implementation!) to implement the connector traits for each flow; stripe/transformers.rs holds the TryFrom implementations that map domain types to and from Stripe's API, matching on PaymentMethodData. To add a processor you create one of these pairs. See transformer, macros, and the new-connector skill."
  },
  "prerequisites": ["unified-request", "what-is-prism"],
  "related": ["transformer", "flow", "macros", "payment-method"],
  "go_deeper": [
    {"path": "crates/integrations/connector-integration/src/connectors/stripe.rs", "why": "a real connector: struct + flow wiring via macros"},
    {"path": "crates/integrations/connector-integration/src/connectors.rs", "why": "the registry listing every connector"},
    {"path": "docs/architecture/README.md", "why": "the connector-adapter pattern explained"}
  ],
  "verify_anchors": [
    {"path": "crates/integrations/connector-integration/src/connectors/stripe.rs", "must_contain": "create_all_prerequisites"},
    {"path": "crates/integrations/connector-integration/src/connectors/stripe/transformers.rs", "must_contain": "PaymentMethodData"}
  ]
}
---
