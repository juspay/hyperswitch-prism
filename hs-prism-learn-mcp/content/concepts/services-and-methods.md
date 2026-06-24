---
{
  "slug": "services-and-methods",
  "title": "Services and methods: how operations are grouped",
  "tier": "core-domain",
  "audience": "engineer",
  "one_liner": "Operations are grouped into services (PaymentService, RefundService, ...) and each service exposes methods (Authorize, Capture, Get, ...) -- this is the API surface flows plug into.",
  "analogy": "Departments in a company. The Payments department handles authorize/capture/void; the Refunds department handles refund/refund-status. Services are departments; methods are the tasks each one does.",
  "depth": {
    "tldr": "The API is organized into services (like PaymentService and RefundService), and each service has methods (like Authorize, Capture, Get). Flow names you see in coverage reports are written as Service.Method, e.g. PaymentService.Authorize.",
    "standard": "A service is a logical grouping of related operations; a method is one operation within it. PaymentService groups Authorize, Capture, Void; RefundService groups refund and refund-status; an Event service handles webhooks. This grouping comes straight from the gRPC service definitions, which is why the coverage matrix labels flows as PaymentService.Authorize or RefundService.Get. Understanding this naming helps you read coverage data and the proto.",
    "deep": "The service/method definitions live in the proto (services.proto) alongside payment.proto. The all_connector coverage report follows these names. See payment-proto and flow."
  },
  "prerequisites": ["flow"],
  "related": ["payment-proto", "flow"],
  "go_deeper": [
    {"path": "docs/architecture/concepts/services-and-methods.md", "why": "the concept explained in the architecture docs"}
  ],
  "verify_anchors": []
}
---
