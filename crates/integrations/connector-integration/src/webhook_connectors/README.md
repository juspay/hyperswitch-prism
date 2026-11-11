# Webhook framework

This directory registers provider implementations of `interfaces::webhook::Webhook`.
No production connector is migrated by this PR. Existing webhook calls continue
through their payment connector's `IncomingWebhook` implementation by default.
Inline test implementations exercise provider dispatch without production registration.

## Registering a provider

1. Implement `Webhook` in `<connector>.rs`, with payload types and event/reference
   mappings in `<connector>/transformers.rs`. Export and register supported provider
   variants in `../webhook_connectors.rs`.
2. Implement `parse_event` for unverified classification/reference extraction.
   Override `get_event_type` if HandleEvent classification must not require references.
3. Implement local signature verification, provider ACKs and integrity declarations
   as appropriate. External gRPC verification is provider-owned: return a static
   `WebhookVerificationIntegration` from `get_external_verification_integration`.
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

`x-webhook-connector` is permanent provider identity. Its current ability to select
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
