# Common webhook operations

This directory contains provider implementations of
`interfaces::webhook_common::WebhookCommon`. ParseEvent extracts the event type
and reference; HandleEvent uses common classification, local source verification,
ACK generation, and integrity-check declarations. Resource normalization remains
in the existing payment connectors, including their payout webhook processing.

The original `interfaces::webhook_parse::WebhookParse` and boxed connector paths
remain compatibility re-exports. Existing parse-only implementations compile
because the additional common methods have defaults.

To implement a provider later:

1. Add `<connector>.rs` implementing `WebhookCommon`, following TrueLayer or Trustly.
2. Define the provider payload types and parsing mappings in
   `<connector>/transformers.rs`. Extract both the event type
   and its reference, including payment, refund, dispute, mandate, or payout IDs.
3. Export the connector module and type from `../webhook_connectors.rs`.
4. Register the provider in `get_connector` in `../webhook_connectors.rs`.
   Match both payment and payout variants when they share the parser. Return
   `Some(Box::new(ProviderWebhook::new()))` for available providers and `None` otherwise.

Both gRPC and FFI call the shared `get_webhook_connector_data` resolver (`get_parse_event_connector_data` remains available).
It returns `ParseEventConnectorData`, and callers match its variant:

- `Webhook(WebhookConnectorData)` uses `Box<dyn WebhookCommon>` for provider-level operations.
- `Payment(ConnectorData)` uses the existing payment connector's `IncomingWebhook` methods.

Header validation, migration policy, and connector construction belong to the
resolver. The parsing entry points only dispatch on the returned data.
Selection happens before parsing:

| Request | Selected implementation |
| --- | --- |
| `x-webhook-connector: trustly` | Standalone Trustly parser, regardless of the migration list |
| Existing connector metadata, provider in `webhook_parse.migrated_connectors` | Standalone provider parser |
| Existing connector metadata, provider absent from the list | Legacy payment connector |

Set `CS__WEBHOOK_PARSE__MIGRATED_CONNECTORS="trustly,truelayer"` to opt in
existing HS requests without changing their `x-connector` header. The default
is an empty list. The native FFI handler honors the same environment variable;
direct service-transformer callers supply the migration configuration themselves.
The new header is accepted by gRPC ParseEvent/HandleEvent and by service transformers receiving metadata.
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

ParseEvent does not authenticate notifications. HandleEvent verifies the source
before resource processing. A false verification result retains its existing
behavior: processing may still return content and an ACK with `source_verified`
set to false. Native FFI retains local verification only; the gRPC external
verification flow stays on the existing payment integration.

Providers should implement `get_event_type` separately when classification must
not require ParseEvent's resource references. This preserves HandleEvent's input
validation. Existing payment methods delegate to the common implementations to
keep their behavior and public interfaces available.

TrueLayer and Trustly are available for both payment and payout variants.
Their shared payload types and parsing mappings live in this directory; the
existing handlers reuse those types through re-exports. Registration alone does
not enable migration for existing headers; the rollout list controls that choice.
