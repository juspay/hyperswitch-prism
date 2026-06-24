---
{
  "slug": "stateless",
  "title": "Stateless: no database, no stored card data",
  "tier": "orientation",
  "audience": "everyone",
  "one_liner": "Prism keeps no database and stores no card data -- a request comes in, a response goes out, and nothing is remembered.",
  "analogy": "A translator at a meeting. They turn each sentence from one language into another, then forget it. They do not keep a transcript. Prism translates each payment request and keeps nothing.",
  "depth": {
    "tldr": "Prism does not save your data. It takes a request, translates and forwards it, returns the answer, and remembers nothing. That keeps it simple and keeps sensitive card data from piling up.",
    "standard": "Stateless means there is no database and no stored state between calls. Each request carries everything needed (including credentials passed in, not stored). Two big benefits: (1) it is simple to reason about and to scale -- any instance can serve any request; (2) because card data is never persisted, your PCI compliance scope stays small. Prism is a transformation layer, not a system of record.",
    "deep": "Connectors hold no per-request state -- they are structs generic over a payment-method type with no stored fields. Credentials arrive in the request config, not from a stored secret. This is a deliberate design choice that pushes durability and storage concerns to the caller. See the connector and compliance docs."
  },
  "prerequisites": ["what-is-prism"],
  "related": ["connector", "three-layer-architecture"],
  "go_deeper": [
    {"path": "README.md", "why": "states the stateless, PCI-scope-reducing design"},
    {"path": "docs/architecture/compliance/compliance.md", "why": "how statelessness reduces PCI scope"}
  ],
  "verify_anchors": [
    {"path": "README.md", "must_contain": "stateless"}
  ]
}
---
