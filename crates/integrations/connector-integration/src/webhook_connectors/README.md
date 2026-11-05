# Webhook parsers

This directory contains provider implementations for the first webhook call, `ParseEvent`.
It uses `interfaces::webhook_parse::WebhookParse`, which takes borrowed
`RequestDetails` and returns the domain `WebhookParseResponse` containing an
`EventType` and an optional `WebhookResourceReference`.

To implement a provider later:

1. Add `<connector>.rs` implementing `WebhookParse`, following TrueLayer or Trustly.
2. Define the provider payload types and parsing mappings in
   `<connector>/transformers.rs`. Extract both the event type
   and its reference, including payment, refund, dispute, mandate, or payout IDs.
3. Export the connector module and type from `../webhook_connectors.rs`.
4. Register the provider in `get_connector` in `../webhook_connectors.rs`.
   Match both payment and payout variants when they share the parser. Return
   `Some(Box::new(ProviderWebhook::new()))` for migrated providers and `None` otherwise.

Both gRPC and FFI resolve `WebhookConnectorData` and call the shared parsing
helper through `Box<dyn WebhookParse>`. Selection happens before parsing:

| Request | Selected implementation |
| --- | --- |
| `x-webhook-connector: trustly` | Standalone Trustly parser, regardless of the migration list |
| Existing connector metadata, provider in `webhook_parse.migrated_connectors` | Standalone provider parser |
| Existing connector metadata, provider absent from the list | Legacy payment adapter |

Set `CS__WEBHOOK_PARSE__MIGRATED_CONNECTORS="trustly,truelayer"` to opt in
existing HS requests without changing their `x-connector` header. The default
is an empty list. The native FFI handler honors the same environment variable;
direct service-transformer callers supply the migration configuration themselves.
The new header is accepted by gRPC and by service transformers receiving metadata.
The public native byte API continues to identify providers through its typed
connector options; it does not expose routing headers.

The explicit header takes precedence over legacy connector identity headers.
If typed connector configuration is supplied, its provider must match the
explicit header. An unknown selector, an unavailable standalone implementation,
or an error from the selected parser is returned without legacy fallback.
Non-payment variants remain unsupported on the legacy path.

The common entry point converts the domain response into the existing
protobuf response. `None` means that the notification has no supported resource
reference; malformed payloads must return a parsing error.

The parser does not authenticate notifications. Its references remain unverified
until the handling phase verifies the source. Verification, acknowledgements,
business context, and status processing remain in the existing handling path.

TrueLayer and Trustly are available for both payment and payout variants.
Their shared payload types and parsing mappings live in this directory; the
existing handlers reuse those types through re-exports. Registration alone does
not enable migration for existing headers; the rollout list controls that choice.
