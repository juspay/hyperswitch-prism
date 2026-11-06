# Common webhook framework

This directory is the registration point for provider implementations of
`interfaces::webhook_common::WebhookCommon`. No connectors are migrated or
registered by this change. Existing webhook calls use their payment connector's
`IncomingWebhook` implementation by default.

## Adding a provider later

1. Add `<connector>.rs` implementing `WebhookCommon`, with shared payload types
   and event/reference mappings in `<connector>/transformers.rs`.
2. Implement `parse_event` to extract the unverified event type and resource
   reference. Override `get_event_type` when HandleEvent classification must not
   require ParseEvent's reference fields.
3. Implement common local source verification, signature/message extraction,
   provider ACKs, and integrity-check declarations where supported. Keep existing
   verification and unverified-response behavior when migrating a provider.
4. Export the provider from `../webhook_connectors.rs` and register its supported
   connector variants in `get_connector`. Payment and payout variants may share
   one provider implementation.
5. Keep payment, refund, and dispute normalization on the payment connector's
   `IncomingWebhook`. For payout normalization, implement the defaulted
   `PayoutServiceTrait::process_payout_webhook` method and register the provider
   in `payout_connectors::get_webhook_connector`. Preserve legacy entry points
   when migrating existing providers.
6. Enable only registered providers in `webhook_parse.migrated_connectors` through
   the normal configuration loader. The environment override is
   `CS__WEBHOOK_PARSE__MIGRATED_CONNECTORS` (comma-separated provider names).

No provider implementation, template, or additional test files are included.
Both registries are intentionally empty until a separate connector migration.

## Routing and compatibility

Both gRPC and FFI use the shared `get_webhook_connector_data` resolver. It returns
`ParseEventConnectorData`, and callers match its variant:

- `Webhook(WebhookConnectorData)` uses `Box<dyn WebhookCommon>`.
- `Payment(ConnectorData)` uses the existing `IncomingWebhook` methods.

Existing connector metadata selects the legacy path unless the provider is in
the migration list. The default list is empty. `x-webhook-connector` explicitly
selects a common provider on gRPC ParseEvent/HandleEvent and service transformers
receiving metadata. It overrides legacy identity headers, but any supplied typed
connector configuration must match its provider. The native byte API continues
using typed connector options and does not expose routing headers.

Explicit selection or configured migration to an unregistered provider returns
an error. With the empty registries in this PR, no common provider can be selected.
Errors from a selected implementation never retry through the legacy path.

HandleEvent uses the selected common provider for classification, local
verification, ACK generation, and integrity declarations. A resource resolver
returns payout data only for a payout event, an active common migration branch,
and a registered payout handler. Other cases retain existing payment dispatch.
The gRPC external verifier retains its payment compatibility context; future
purely payout-only providers need that context addressed separately. Native FFI
retains local verification only.

ParseEvent does not authenticate notifications. Existing HandleEvent behavior
allows content and an ACK with `source_verified=false` after a failed source check.
The original resolver name, `process_webhook_event` helper, `IncomingWebhook`
methods, and `WebhookParse` compatibility trait path remain available.
