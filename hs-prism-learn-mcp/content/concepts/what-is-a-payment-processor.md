---
{
  "slug": "what-is-a-payment-processor",
  "title": "What a payment processor is",
  "tier": "payments-primer",
  "audience": "everyone",
  "one_liner": "A payment processor (Stripe, Adyen, PayPal, Razorpay...) is the company that actually moves money between a shopper's bank and a merchant's bank.",
  "analogy": "A courier for money. The shop hands the courier a package (the payment request); the courier knows how to reach each bank and deliver it. Every courier has its own forms and rules.",
  "depth": {
    "tldr": "A payment processor is the service that talks to the banks and card networks to move money. Stripe, Adyen, and PayPal are processors. The problem: each one has a different API, so connecting to many of them is a lot of repeated work.",
    "standard": "A payment processor connects merchants to the card networks (Visa, Mastercard) and banks. Each processor exposes its own API with its own field names, authentication, status codes, and quirks. A business that wants choice, better pricing, or regional coverage ends up integrating several processors -- and each integration is weeks of bespoke work. That repeated pain is exactly the problem hyperswitch-prism solves: one unified way to talk to all of them.",
    "deep": "In this repo each processor is represented by a 'connector' -- a small module that translates the repo's one unified request into that processor's exact API shape, and translates the reply back. There are ~95 connectors, all built to the same pattern. See the connector and why-prism-exists cards."
  },
  "prerequisites": ["payments-101"],
  "related": ["what-is-prism", "why-prism-exists", "connector"],
  "go_deeper": [
    {"path": "README.md", "why": "the project overview and the list of supported processors"},
    {"path": "docs/architecture/README.md", "why": "how a processor maps to a connector"}
  ],
  "verify_anchors": []
}
---
