---
{
  "slug": "transformer",
  "title": "Transformer: the request/response translator",
  "tier": "core-domain",
  "audience": "engineer",
  "one_liner": "A transformer is the code inside a connector that converts Prism's unified request into the processor's exact API shape, and converts the processor's response back.",
  "analogy": "A bilingual interpreter. You speak the unified language; the interpreter (transformer) re-says it in the processor's language, then re-says the reply back to you.",
  "depth": {
    "tldr": "The transformer is the part of a connector that does the actual translating: unified request -> processor's request, and processor's response -> unified response. It lives in the connector's transformers.rs file.",
    "standard": "In Rust, transformers are TryFrom implementations: one direction builds the processor's request type from the unified RouterDataV2, the other builds the unified response from the processor's reply. They match on PaymentMethodData (Card, Wallet, BankTransfer, ...) to handle each payment method, and they map the processor's status strings to Prism's numeric status codes. Most of a connector's real logic lives here.",
    "deep": "See stripe/transformers.rs: it defines request/response types (e.g. StripeAuthorizeRequest/Response), implements TryFrom<RouterDataV2<...>> for each, and matches PaymentMethodData arms. Status mapping is done via From/TryFrom, never hardcoded outside match arms. The naming convention is ConnectorNameFlowRequest / ConnectorNameFlowResponse. See connector, router-data-v2, payment-method, and status-codes."
  },
  "prerequisites": ["connector", "unified-request"],
  "related": ["router-data-v2", "payment-method", "status-codes", "domain-types"],
  "go_deeper": [
    {"path": "crates/integrations/connector-integration/src/connectors/stripe/transformers.rs", "why": "real TryFrom transformers matching on PaymentMethodData"},
    {"path": ".skills/_shared/references/type-system.md", "why": "the types a transformer maps between"}
  ],
  "verify_anchors": [
    {"path": "crates/integrations/connector-integration/src/connectors/stripe/transformers.rs", "must_contain": "PaymentMethodData"}
  ]
}
---
