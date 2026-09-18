---
{
  "slug": "status-codes",
  "title": "Status codes are numbers, not strings",
  "tier": "operational",
  "audience": "engineer",
  "one_liner": "A payment's status is a NUMBER (e.g. CHARGED = 8), not a string -- and a decline comes back as a FAILURE status inside a normal response, not as a thrown error.",
  "analogy": "A traffic light by code, not by word. The system reports '8' (charged) the way a light reports a coded signal -- you compare against the known codes, you do not read a sentence.",
  "depth": {
    "tldr": "response.status is a number. Key payment codes: AUTHORIZED = 6, CHARGED = 8, VOIDED = 11, PENDING = 20, FAILURE = 21. A declined payment is FAILURE in the response body -- check the number, do not expect an exception.",
    "standard": "Status comes from the proto's numeric enums (PaymentStatus, RefundStatus). Compare against the named constants, not strings. Common PaymentStatus values: AUTHORIZED (6), CHARGED (8), VOIDED (11), PENDING (20), FAILURE (21). A soft decline (insufficient funds, etc.) is a normal response with status FAILURE -- it does not throw. Hard failures (network, integration bugs) are the ones that throw. This distinction is the heart of correct error handling.",
    "deep": "The numeric mappings are defined in payment.proto's PaymentStatus / RefundStatus enums (the source of truth). Connectors map a processor's status strings to these numbers via From/TryFrom, never hardcoding outside match arms. See error-model and error-handling.md."
  },
  "prerequisites": ["flow"],
  "related": ["error-model", "flow-authorize", "transformer"],
  "go_deeper": [
    {"path": "docs/architecture/concepts/error-handling.md", "why": "status vs error, with examples"},
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "why": "the PaymentStatus / RefundStatus numeric enums, the source of truth"}
  ],
  "verify_anchors": [
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "must_contain": "PaymentStatus"}
  ]
}
---
