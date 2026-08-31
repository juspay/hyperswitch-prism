---
{
  "slug": "why-prism-exists",
  "title": "Why hyperswitch-prism exists",
  "tier": "payments-primer",
  "audience": "everyone",
  "one_liner": "Every payment processor has a different API, so connecting to many of them is slow, repetitive, and error-prone -- Prism removes that by giving you one API for all of them.",
  "analogy": "Power sockets differ by country. Without a universal adapter you would carry a drawer full of plugs. Prism is the one adapter that replaces the drawer.",
  "depth": {
    "tldr": "Without Prism, supporting five processors means learning and maintaining five different APIs. With Prism, you learn one. That is the whole point.",
    "standard": "Businesses use multiple processors for better pricing, redundancy, and regional coverage. But each processor's API is different -- different fields, auth, status codes, and edge cases -- so every new one is weeks of bespoke integration plus ongoing maintenance as their API changes. Prism collapses that into a single unified interface: one request shape, one response shape, one set of status codes, for all 95+ processors. Add a processor once (a connector), and every app using Prism can use it.",
    "deep": "Prism was extracted from Juspay Hyperswitch's production connectors, hardened over years of real traffic. The cost of integration is moved from every app to one shared, well-tested place. New connectors are largely code-generated (see the grace card) and reviewed against a strict checklist, so quality stays high as breadth grows."
  },
  "prerequisites": ["what-is-a-payment-processor"],
  "related": ["what-is-prism", "unified-request", "grace"],
  "go_deeper": [
    {"path": "docs/blogs/why-we-built-a-unified-payment-integration-library.md", "why": "the motivation, in the team's own words"},
    {"path": "README.md", "why": "the problem statement and goals"}
  ],
  "verify_anchors": []
}
---
