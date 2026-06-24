---
{
  "slug": "money-struct",
  "title": "Money is carried in minor units",
  "tier": "operational",
  "audience": "engineer",
  "one_liner": "Amounts are integers in the currency's minor unit -- 1000 means 10.00 USD (cents), not one thousand dollars. Money pairs that integer with a currency.",
  "analogy": "Counting in cents, not dollars. A cashier who counts only whole cents never loses a fraction. Storing 1000 cents is exact; storing 10.00 as a float is not.",
  "depth": {
    "tldr": "Amounts are whole numbers of the smallest currency unit. 1000 + USD = 10.00 dollars. This avoids rounding bugs from decimals. Always pass minor units.",
    "standard": "The Money type pairs a minor_amount (an integer in the currency's smallest unit -- cents for USD) with a currency. Using integers in minor units avoids floating-point rounding errors that plague money math. A frequent newcomer mistake is sending 10 expecting ten dollars; for USD you must send 1000. Currencies with different minor-unit scales are handled by the currency field.",
    "deep": "Money is defined in payment.proto (message Money { minor_amount, currency }) and explained in the money-struct framework doc. Connectors pass minor units straight through to processors that expect them. See payment-proto and unified-request."
  },
  "prerequisites": ["unified-request"],
  "related": ["payment-proto", "unified-request"],
  "go_deeper": [
    {"path": "docs/architecture/frameworks/money-struct.md", "why": "the Money framework, in depth"},
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "why": "the Money message with minor_amount"}
  ],
  "verify_anchors": [
    {"path": "crates/types-traits/grpc-api-types/proto/payment.proto", "must_contain": "minor_amount"}
  ]
}
---
