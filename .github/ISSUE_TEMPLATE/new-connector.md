---
name: New medusa-plugin connector
about: Track adding a new payment connector to the Medusa plugin (plugins/medusa)
title: "feat(medusa/connector): add <connector-name>"
labels: ["connector", "enhancement", "medusa"]
---

> Scope: the **Medusa payment plugin** under [`plugins/medusa`](../../plugins/medusa) —
> the backend package `medusa-unified-payment` and the React package
> `medusa-unified-payment-react`. All paths below are relative to the monorepo root.

## Connector

- **Name** (lowercase id used in code, e.g. `stripe`): <!-- e.g. razorpay -->
- **Provider id**: `pp_hyperswitch-prism_<name>`
- **Client SDK** (script/npm package, if any):
- **UCS support**: does hyperswitch-prism (this monorepo) support this connector? webhooks? (adyen/paypal only today)

## Capabilities

- [ ] initiate (required)
- [ ] authorize
- [ ] capture
- [ ] refund
- [ ] void/cancel
- [ ] webhooks (only if UCS supports it)
- [ ] async authorize (outcome arrives by webhook, like Adyen)
- [ ] tokenization / re-initiate flow (like GlobalPay)

> Reference an existing connector with the closest flow: **stripe** (sync, client_secret),
> **adyen** (async, webhook-driven authorize), **paypal** (orders + webhook),
> **globalpay** (access-token + card tokenization).

## Backend — `plugins/medusa/medusa-unified-payment`

- [ ] `plugins/medusa/medusa-unified-payment/src/providers/connector/prism-<name>/index.ts` —
      implement `initiatePayment` (+ `authorizePayment`/`refundPayment`/`handleWebhook`/`reInitiatePayment`
      if custom). Reuse helpers from `providers/utils` and, for webhooks, `connector/webhook-common.ts`.
- [ ] `plugins/medusa/medusa-unified-payment/src/providers/connector/prism-<name>/README.md` —
      document the flow (mirror an existing one).
- [ ] `plugins/medusa/medusa-unified-payment/src/providers/connector/index.ts` —
      `export * as <name> from "./prism-<name>"`.
- [ ] `plugins/medusa/medusa-unified-payment/src/providers/prism.ts` — add `case "<name>"` to the
      `initiatePayment` switch; add connector branches in `authorizePayment` / `refundPayment` /
      void / `handleWebhook` **only if** the connector needs custom logic.
- [ ] `plugins/medusa/medusa-unified-payment/src/providers/connector/types.ts` — extend connector
      union / context types if needed.
- [ ] `plugins/medusa/medusa-unified-payment/src/providers/types.ts` — add the connector's
      `connectorConfig` shape (credentials; plus optional `publishableKey` if surfaced to the storefront).
- [ ] Build passes (`cd plugins/medusa/medusa-unified-payment && npm run build`); unit tests
      added/updated where applicable.

## React — `plugins/medusa/medusa-unified-payment-react`

- [ ] `plugins/medusa/medusa-unified-payment-react/src/connectors/<name>/<Name>Wrapper.tsx` —
      React component (SDK mount + onSubmit/onError).
- [ ] `plugins/medusa/medusa-unified-payment-react/src/connectors/<name>/utils.ts` — lazy SDK
      loader (if the connector ships a script).
- [ ] `plugins/medusa/medusa-unified-payment-react/src/connectors/<name>/index.ts` — export the
      `PaymentConnector` object and/or the Wrapper.
- [ ] `plugins/medusa/medusa-unified-payment-react/src/connectors/<name>/README.md` — document
      the wrapper + session data consumed.
- [ ] `plugins/medusa/medusa-unified-payment-react/src/connectors/index.ts` — register in the
      `connectors` map (if using the generic path).
- [ ] `plugins/medusa/medusa-unified-payment-react/src/index.ts` — export the connector object
      (if any) and the Wrapper.
- [ ] `plugins/medusa/medusa-unified-payment-react/src/utils/predicates.ts` — add
      `isHyperswitchPrism<Name>`, include it in `isHyperswitchPrism`, and add the id to
      `HYPERSWITCH_PRISM_PROVIDER_IDS`.
- [ ] Build + lint pass (`cd plugins/medusa/medusa-unified-payment-react && npm run build && npm run lint`).

## Tests & docs (`plugins/medusa`)

- [ ] Add/extend an E2E spec under `plugins/medusa/app/e2e/ui/<name>.spec.ts` (gated on
      `hasConnector("<name>")`); the `e2e-medusa` workflow runs it.
- [ ] Update `plugins/medusa/medusa-unified-payment/README.md` (config example, credentials &
      support matrix) and `plugins/medusa/creds.example.json`.

## Acceptance criteria

- [ ] A payment can be initiated, authorized, captured, refunded, and voided (per capabilities).
- [ ] Both packages build with no type errors.
- [ ] READMEs document the flow and any required `connectorConfig` / credentials keys.
