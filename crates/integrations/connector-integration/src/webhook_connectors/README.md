# Webhook parser template

This directory contains the scaffold for the first webhook call, `ParseEvent`.
It uses `interfaces::webhook_parse::WebhookParse`, which takes borrowed
`RequestDetails` and returns the domain `WebhookParseResponse` containing an
`EventType` and an optional `WebhookResourceReference`.

To implement a provider later:

1. Copy `template.rs.template` to `<connector>.rs` and
   `template/transformers.rs.template` to `<connector>/transformers.rs`.
2. Replace `{{CONNECTOR_NAME_PASCAL}}` with the provider's Rust type name.
3. Define the provider payload types and replace the unsupported parsing stub
   in `transformers.rs`. Parse the request once to extract both the event type
   and its reference, including payment, refund, dispute, mandate, or payout IDs.
4. Export the connector module and type from `../webhook_connectors.rs`.
5. Register the provider in `get_connector` in `../webhook_connectors.rs`.
   Match both payment and payout variants when they share the parser. Return
   `Some(Box::new(ProviderWebhook::new()))` for migrated providers and `None` otherwise.

Both gRPC and FFI resolve `WebhookConnectorData` and call the shared parsing
helper. Registered providers use their standalone parser; other payment
connectors use a legacy adapter behind the same `Box<dyn WebhookParse>`.
Unregistered non-payment variants remain unsupported. A registered parser's
errors propagate without retrying through the legacy adapter.

The common entry point converts the domain response into the existing
protobuf response. `None` means that the notification has no supported resource
reference; malformed payloads must return a parsing error.

The parser does not authenticate notifications. Its references remain unverified
until the handling phase verifies the source. Verification, acknowledgements,
business context, and status processing remain in the existing handling path.

TrueLayer and Trustly are registered for both payment and payout variants.
Their shared payload types and parsing mappings live in this directory; the
existing handlers reuse those types through re-exports. Other payment providers
continue through the legacy adapter. The `.template` files are not compiled Rust modules.
