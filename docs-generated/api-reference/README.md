# API Reference

A complete reference for all Prism API services and request/response types.

## Services

Prism provides a unified gRPC API for payment processing across 60+ payment processors. The API is organized into services that handle different aspects of the payment lifecycle.

| Service | Description | Key Operations |
|---------|-------------|----------------|
| [Payment Service](./services/payment-service/) | Core payment lifecycle | Authorize, Capture, Void, Refund, CreateOrder |
| [Refund Service](./services/refund-service/) | Refund operations | Get refund status |
| [Recurring Payment Service](./services/recurring-payment-service/) | Stored payment methods | Charge, Revoke mandate |
| [Dispute Service](./services/dispute-service/) | Chargeback handling | Accept, Defend, SubmitEvidence |
| [Event Service](./services/event-service/) | Webhook processing | Handle connector events |
| [Customer Service](./services/customer-service/) | Customer management | Create customer |
| [Payment Method Service](./services/payment-method-service/) | Payment method storage | Tokenize |
| [Payment Method Authentication Service](./services/payment-method-authentication-service/) | 3DS authentication | Pre-authenticate, Authenticate, Post-authenticate |
| [Merchant Authentication Service](./services/merchant-authentication-service/) | Session management | CreateAccessToken, CreateSessionToken, CreateSdkSessionToken |