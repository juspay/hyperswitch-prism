# Webhook framework

This directory holds standalone provider implementations of `interfaces::webhook::Webhook`.
The sibling `../webhook_connectors.rs` declares their modules and registers them.
No production connector is migrated by this PR. Existing webhook calls continue
through their payment connector's `IncomingWebhook` implementation by default.
Inline test implementations exercise provider dispatch without production registration.

## Registering a provider

All paths below are relative to `crates/integrations/connector-integration/src/`.
`paypal` is an illustrative provider name; this PR does not create or migrate it.

| Responsibility | Trait / method | Implementation file |
| --- | --- | --- |
| Supply a representative request for field-probe validation | `Webhook::sample_webhook_request` (required) | `webhook_connectors/paypal.rs` |
| Parse event type and resource references | `Webhook::parse_event` | `webhook_connectors/paypal.rs` |
| Classify an event during HandleEvent | `Webhook::get_event_type` | `webhook_connectors/paypal.rs` |
| Verify the source locally | `Webhook::verify_webhook_source` | `webhook_connectors/paypal.rs` |
| Supply the external verification integration | `Webhook::get_external_verification_integration`, `requires_external_webhook_verification` | `webhook_connectors/paypal.rs` |
| Construct the provider ACK | `Webhook::get_webhook_api_response` | `webhook_connectors/paypal.rs` |
| Declare supported integrity checks | `Webhook::get_webhook_integrity_checks` | `webhook_connectors/paypal.rs` |
| Define webhook payloads and event/reference mappings | Provider-specific types and conversions | `webhook_connectors/paypal/transformers.rs` |
| Build unified payment/refund/dispute content | `IncomingWebhook::process_payment_webhook`, `process_refund_webhook`, `process_dispute_webhook` | `connectors/paypal.rs` and its transformers |
| Build unified payout content, when supported | `PayoutServiceTrait::process_payout_webhook` | `payout_connectors/paypal.rs` and its transformers |
| Declare provider modules and register common webhook implementations | `webhook_connectors::get_connector` | `webhook_connectors.rs` |
| Register payout resource handlers, when supported | `payout_connectors::get_webhook_connector` | `payout_connectors.rs` |

The new `Webhook` implementation belongs in `webhook_connectors/<provider>.rs`,
separate from the payment connector in `connectors/<connector>.rs`. Use a standalone
webhook provider type so common webhook operations can run without constructing a
payment or payout connector. `webhook_connectors.rs` is the module/registry entry
point; provider behavior belongs in its per-provider files. The payout implementation
can use a distinct Rust type (for example `PaypalPayouts`). Register it under the
matching `PayoutConnectorEnum` variant; the handler resolver uses the provider name
to find that variant after event classification.

1. Implement `Webhook` in `webhook_connectors/<provider>.rs`, with payload types
   and event/reference mappings in `webhook_connectors/<provider>/transformers.rs`.
   Declare the module and add the provider name/factory to `PROVIDERS` in
   `webhook_connectors.rs`. Runtime lookup and field-probe enumeration share this registry.
2. Implement the required `sample_webhook_request` with a parseable body and any
   required headers/URI. The field probe calls `parse_event` on this request for
   every registered provider, including payout-only providers; parsing errors are
   reported as errors, never as supported. Implement `parse_event` for unverified
   classification/reference extraction.
   Override `get_event_type` if HandleEvent classification must not require references.
3. Implement local signature verification, provider ACKs and integrity declarations
   as appropriate. External gRPC verification is provider-owned: return a borrowed
   `WebhookVerificationIntegration<'_>` from `get_external_verification_integration`.
   A provider implementing the verification integration itself can return
   `Some(Box::new(self))`; its borrow lasts through the awaited HTTP call.
   Override `requires_external_webhook_verification` for providers that always need
   an external call. The existing server verification list is also honored. Required
   external verification without an integration returns an error, never local fallback.
4. Keep payment/refund/dispute normalization in the payment connector's `IncomingWebhook`.
   Implement payout normalization in `PayoutServiceTrait::process_payout_webhook` and
   register it in `payout_connectors::get_webhook_connector`, keyed by `PayoutConnectorEnum`.
5. Add the provider to `webhook_parse.migrated_connectors` in development/production
   TOML when ready. gRPC also supports the normal server environment override
   `CS__WEBHOOK_PARSE__MIGRATED_CONNECTORS`. FFI embeds the selected TOML at build time;
   it retains its existing loader and does not apply server environment overrides.
   Both transports reject unregistered rollout entries while loading configuration.

## Identity and migration lifecycle

`x-webhook-connector` is permanent provider identity. Without typed configuration,
metadata keeps it as `WebhookProvider`, without trying payment/payout enums in
order. Provider registry lookup uses this identity directly. Typed configuration
continues to identify its payment/payout family and must match the provider name. Its current ability to select
provider dispatch ahead of migration is temporary. Supplied typed configuration
must match this identity. The header overrides legacy identity metadata when no
typed connector configuration is supplied. Native byte APIs continue using typed
connector options, without a routing header.

The resolver returns `ParseEventConnectorData::Webhook` or `::Payment` during
migration. Configured or explicit selection of an unregistered provider returns
an error without retrying legacy execution. Both registries are empty in this PR.

The final connector-migration PR owns cleanup of the rollout config, legacy resolver/handler branches,
compatibility aliases and special migration selection once all supported connectors
have migrated and their parse/handle compatibility checks pass. The permanent
provider header, webhook trait and event-based resource dispatch remain. The first
provider migration must add real-provider coverage alongside these framework tests.

## HandleEvent

The legacy branch retains verification, classification, resource processing and
ACK ordering. The provider branch is separate:

1. Classify the event through the webhook provider.
2. Resolve the resource using provider identity and event type. Payout events use
   the payout registry directly; missing registration fails without payment fallback.
   Payment/refund/dispute events resolve payment data only at this stage.
3. Verify the source using the provider's local verifier or gRPC external integration.
4. Normalize through the selected resource handler and generate the provider ACK.

Provider payout handling and external verification do not require payment connector
data. FFI retains local verification only and its existing empty integrity declarations.
Classification runs once in HandleEvent; resource normalization may deserialize the
raw payload again. Failed local verification can still produce content and an ACK
marked `source_verified=false`, preserving existing response semantics. Normalization
errors stop before ACK generation; handler/ACK errors never retry another path.

## Layer boundaries

`ucs_interface_common` parses metadata without importing concrete connector registries.
`connector-integration::webhook_dispatch` owns registry lookup and rollout registration
validation. Transports pass the migration decision and optional explicit provider name
into the resolver, then match its legacy/provider result. The external verification
adapter is temporary (`verify_webhook_source_external_legacy`); the shared executor
(`verify_webhook_source_external`) remains after migration.
