---
{
  "slug": "payment-method",
  "title": "Payment method: how the customer pays",
  "tier": "core-domain",
  "audience": "everyone",
  "one_liner": "A payment method is the way a customer pays -- card, wallet (Apple Pay, Google Pay), bank transfer, UPI, or buy-now-pay-later -- and each connector decides which ones it supports.",
  "analogy": "Ways to pay at a checkout counter: cash, card, phone tap, gift voucher. Each shop accepts some subset. A connector is a shop deciding which it accepts.",
  "depth": {
    "tldr": "Payment methods are the forms of payment: cards, wallets, bank transfers, UPI, BNPL, and more. In code they appear as branches the connector handles (a Card branch, a Wallet branch, ...).",
    "standard": "Prism supports many payment-method categories: Card, Wallet (Apple Pay, Google Pay, PayPal), Bank Transfer, Bank Redirect, BNPL (Klarna, Afterpay), UPI, and others. In a connector's transformer this shows up as a match on PaymentMethodData with one arm per category. Adding support for a new payment method to an existing connector means adding the matching arm and mapping its fields -- that is exactly what the add-payment-method skill does.",
    "deep": "The PaymentMethodData type is defined in crates/types-traits/domain_types/src/payment_method_data.rs. Connectors must not silently drop unsupported methods -- they return a NotImplemented error with a message. See transformer and the add-payment-method skill; category mappings are in that skill's references."
  },
  "prerequisites": ["payments-101"],
  "related": ["transformer", "connector"],
  "go_deeper": [
    {"path": "docs/getting-started/payment-methods/README.md", "why": "the supported payment methods, explained"},
    {"path": "crates/types-traits/domain_types/src/payment_method_data.rs", "why": "the PaymentMethodData type connectors match on"},
    {"path": ".skills/add-payment-method/SKILL.md", "why": "how to add a payment method to a connector"}
  ],
  "verify_anchors": []
}
---
