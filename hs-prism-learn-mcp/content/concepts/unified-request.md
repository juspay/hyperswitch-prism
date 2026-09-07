---
{
  "slug": "unified-request",
  "title": "The unified request: one shape for every processor",
  "tier": "orientation",
  "audience": "everyone",
  "one_liner": "You build a payment request in ONE format; Prism rewrites it into whatever each processor expects, so your code never changes when you switch processors.",
  "analogy": "Filling out one standard form. You complete a single form; a clerk copies your answers onto each bank's own paperwork. You never learn the banks' forms.",
  "depth": {
    "tldr": "There is one request format for all processors. You fill it once. Prism translates it to Stripe's format, or Adyen's, or PayPal's, behind the scenes. Switching processors does not change your code.",
    "standard": "The unified request is the single typed schema your app fills in -- amount, currency, payment method, capture method, and so on -- regardless of which processor will handle it. A connector's transformer maps that unified request onto the processor's specific API, and maps the processor's response back to the unified response. This is what makes 'write once, use any processor' real. The schema is defined in payment.proto.",
    "deep": "Because the request and response are defined once in the proto, all SDKs and all connectors agree on the same fields and the same status codes. Money is carried in minor units (see money-struct). The mapping table in the architecture doc shows how unified fields land in different processors' requests."
  },
  "prerequisites": ["what-is-prism"],
  "related": ["payment-proto", "transformer", "connector", "money-struct"],
  "go_deeper": [
    {"path": "docs/architecture/README.md", "why": "the unified-to-connector field mapping"},
    {"path": "docs/rfcs/unified-payment-protocol-spec.md", "why": "the unified protocol spec"}
  ],
  "verify_anchors": []
}
---
