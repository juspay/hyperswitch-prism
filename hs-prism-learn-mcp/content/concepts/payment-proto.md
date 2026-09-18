---
{
  "slug": "payment-proto",
  "title": "payment.proto: the contract everything agrees on",
  "tier": "core-domain",
  "audience": "engineer",
  "one_liner": "payment.proto is the single typed schema -- the contract -- that defines the unified request and response that every SDK and every connector must speak.",
  "analogy": "A standard shipping-container spec. Ships, cranes, and trucks worldwide agree on one container size, so anything fits anywhere. payment.proto is that agreed-upon spec for payment messages.",
  "depth": {
    "tldr": "payment.proto is the rulebook for what a payment request and response look like. Every part of the system -- SDKs, the server, connectors -- must match it. It is the source of truth, so nothing has to guess field names.",
    "standard": "payment.proto is a Protocol Buffers (gRPC) schema defining the unified message types: Money (amount + currency), payment method data, request and response shapes, error structures, and the numeric status enums. Because it is defined once, all SDKs and connectors stay consistent automatically. Tools that read this proto (like the integration-mcp) can list exact connector fields and status codes without hallucinating.",
    "deep": "It lives at crates/types-traits/grpc-api-types/proto/payment.proto; the gRPC service methods are in services.proto. Money is carried in minor units via the Money message. Status enums (PaymentStatus, RefundStatus) are numeric -- see status-codes. Connector credential fields are derived from per-connector Config messages. See unified-request and specs-and-dsl."
  },
  "prerequisites": ["unified-request"],
  "related": ["unified-request", "money-struct", "status-codes", "domain-types"],
  "go_deeper": [
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "why": "the actual contract: messages, enums, and money"},
    {"path": "docs/architecture/concepts/specs-and-dsl.md", "why": "how the proto schema (DSL) shapes the typed API"}
  ],
  "verify_anchors": [
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "must_contain": "message Money"}
  ]
}
---
