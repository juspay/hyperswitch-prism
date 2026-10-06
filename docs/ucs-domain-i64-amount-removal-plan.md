# Plan: Remove `i64` Amount Fields From UCS Domain And Proto

**Repo:** `juspay/connector-service`  
**Status:** Planning + first domain cleanup slice in progress; domain amount-like `i64` fields removed for the current slice  
**Primary goal:** remove raw `i64` amount fields from domain types first, then remove or reserve proto `int64` amount fields after compatible `Money` replacements exist and callers have migrated.

---

## Current Code Slice

Removed from domain so far:

- `SetupMandateRequestData.amount: Option<i64>` -> use `minor_amount: Option<MinorUnit>`.
- `PaymentsCaptureData.amount_to_capture: i64` -> use `minor_amount_to_capture: MinorUnit`.
- `RefundsData.payment_amount: i64` -> use `minor_payment_amount: MinorUnit`.
- `RefundsData.refund_amount: i64` -> use `minor_refund_amount: MinorUnit`.
- `PaymentFlowData.amount_captured: Option<i64>` -> use `minor_amount_captured: Option<MinorUnit>`.
- `WebhookDetailsResponse.amount_captured: Option<i64>` -> use `minor_amount_captured: Option<MinorUnit>`.
- `PaymentsCancelData.amount: Option<i64>` -> use `minor_amount: Option<MinorUnit>`.
- `RepeatPaymentData.amount: i64` -> use `minor_amount: MinorUnit`.
- `AuthorizationRequest.order_tax_amount: Option<i64>` -> converted to `Option<MinorUnit>`.
- `AuthorizationRequest.shipping_cost: Option<i64>` -> converted to `Option<MinorUnit>`.
- `SetupRecurringRequest.order_tax_amount: Option<i64>` -> converted to `Option<MinorUnit>`.
- `SetupRecurringRequest.shipping_cost: Option<i64>` -> converted to `Option<MinorUnit>`.
- `RepeatPaymentIntegrityObject.amount: i64` -> converted to `MinorUnit`.

Proto fields are unchanged in this slice.

L2/L3 domain check:

- `OrderDetailsWithAmount.amount`, `total_tax_amount`, and `unit_discount_amount` already use `MinorUnit`.
- `L2L3Data.order_info.discount_amount`, `shipping_cost`, and `duty_amount` already use `MinorUnit`.
- `L2L3Data.tax_info.shipping_amount_tax` and `order_tax_amount` already use `MinorUnit`.
- Proto-to-domain L2/L3 conversion keeps wrapping raw proto `int64` fields at the boundary with `MinorUnit::new(...)`.
- Setup-recurring domain conversion now maps `shipping_cost` from `SetupRecurringRequest.shipping_cost`; it no longer incorrectly sources it from `order_tax_amount`.

---

## Direction

Do not use `legacy_amount_as_i64` as the migration strategy.

The cleanup should move from the external contract inward:

1. Inventory proto amount fields.
2. Map each proto field to the domain field populated by `domain_types/src/types.rs`.
3. Classify the domain field by whether it can be represented as `Money`, `MinorUnit` with a parent currency, or a semantic holdout.
4. Add any missing typed alternative before deleting a raw `i64`.
5. Remove raw `i64` from domain structs first.
6. Keep deprecated proto fields populated only at the proto boundary while compatibility requires it.
7. Remove or reserve proto fields in a later compatibility phase.

Connector code must not read or write deprecated raw amount fields. Connector code should use typed domain amount fields and connector-facing amount converters.

---

## Inventory Method

Build and maintain this table before every implementation PR:

| Proto message.field | Proto type | Deprecated? | Replacement proto field | Domain struct.field | Domain type | Target type | Bucket | Notes |
|---|---:|---:|---|---|---|---|---|---|

Rules:

- Start from `crates/types-traits/grpc-api-types/proto/payment.proto`.
- Include every field whose name or type indicates money: `amount`, `money`, `captured`, `capturable`, `authorized`, `tax`, `cost`, `fee`, `balance`, `commission`, `settlement`.
- For each proto field, trace its converter in `crates/types-traits/domain_types/src/types.rs`.
- For each domain field, list readers and writers before changing it.
- Treat `[deprecated = true]` proto fields separately from non-deprecated raw `int64` fields.

---

## Initial Proto-To-Domain Mapping

This is the first inventory pass for the main payment, refund, setup-recurring, repeat-payment, mandate, and L2/L3 amount fields.

| Proto message.field | Proto type | Deprecated? | Replacement proto field | Domain struct.field | Domain type | Target type | Bucket | Notes |
|---|---:|---:|---|---|---|---|---|---|
| `MandateAmountData.amount` | `optional int64` | yes | `amount_money` | `mandates::MandateAmountData.amount` | `Money` | `Money` | 2 | Domain is already typed; keep deprecated fallback only while old callers can send amount/currency without `amount_money`. |
| `MandateAmountData.currency` | `optional Currency` | yes | `amount_money.currency` | `mandates::MandateAmountData.amount.currency` | `Currency` inside `Money` | `Money` | 2 | Same compatibility gate as `MandateAmountData.amount`. |
| `MandateAmountData.amount_money` | `optional Money` | no | n/a | `mandates::MandateAmountData.amount` | `Money` | `Money` | 2 | Canonical domain source. |
| `MandateAmountData.initial_billing_amount` | `optional Money` | no | n/a | `mandates::MandateAmountData.initial_billing_amount` | `Option<Money>` | `Option<Money>` | 2 | Already typed. |
| `PaymentServiceAuthorizeRequest.amount` | `Money` | no | n/a | `PaymentsAuthorizeData.amount` / `minor_amount` / `currency` | `MinorUnit` + `Currency` | `MinorUnit` + parent currency | 5 | Request amount is parent-currency data in domain. |
| `PaymentServiceAuthorizeRequest.order_tax_amount` | `optional int64` | no | none | `AuthorizationRequest.order_tax_amount` -> `PaymentsAuthorizeData.order_tax_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` or proto `Money` replacement | 4 / 5 | Proto remains raw; domain wrapper converts at the boundary. |
| `PaymentServiceAuthorizeRequest.shipping_cost` | `optional int64` | no | none | `AuthorizationRequest.shipping_cost` -> `PaymentsAuthorizeData.shipping_cost` | `Option<MinorUnit>` | `Option<MinorUnit>` or proto `Money` replacement | 4 / 5 | Proto remains raw; domain wrapper converts at the boundary. |
| `PaymentServiceAuthorizeRequest.surcharge_amount` | `optional Money` | no | n/a | `PaymentsAuthorizeData.surcharge_amount` | `Option<Money>` | `Option<Money>` | 5 | Already typed because surcharge may be independent of the base amount. |
| `PaymentServiceAuthorizeResponse.captured_amount` | `optional int64` | no | none | `PaymentFlowData.amount_captured` | `Option<i64>` | typed source required | 3 / 4 / 6 | Domain mirror should be removed; proto needs replacement or boundary-only compatibility derivation. |
| `PaymentServiceAuthorizeResponse.capturable_amount` | `optional int64` | no | none | `PaymentFlowData.minor_amount_capturable` | `Option<MinorUnit>` | typed source required | 4 / 6 | Domain is typed; proto is raw. |
| `PaymentServiceAuthorizeResponse.authorized_amount` | `optional int64` | yes | `authorized_money` | `PaymentFlowData.minor_amount_authorized` / request integrity | `Option<MinorUnit>` / `Money` response | `Money` boundary field | 2 | Deprecated proto compatibility only. |
| `PaymentServiceAuthorizeResponse.authorized_money` | `optional Money` | no | n/a | generated from authorize integrity object | `Money` | `Money` | 2 | Canonical proto replacement for `authorized_amount`. |
| `PaymentServiceGetResponse.amount` | `optional Money` | no | n/a | `PaymentFlowData.amount` | `Option<Money>` | `Option<Money>` | 5 / 6 | Already typed when response amount is known. |
| `PaymentServiceGetResponse.captured_amount` | `optional int64` | no | none | `PaymentFlowData.amount_captured` | `Option<i64>` | typed source required | 3 / 4 / 6 | Same captured-amount response problem as authorize. |
| `PaymentServiceCaptureRequest.amount_to_capture` | `Money` | no | n/a | `PaymentsCaptureData.amount_to_capture` + `minor_amount_to_capture` + `currency` | `i64` + `MinorUnit` + `Currency` | `MinorUnit` + parent currency | 1 / 5 | Remove domain `i64`; proto is already typed. |
| `PaymentServiceCaptureRequest.order_tax_amount` | `optional Money` | no | n/a | `PaymentsCaptureData.order_tax_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` + capture currency | 5 | Proto carries currency; domain stores parent-currency minor amount. |
| `PaymentServiceCaptureResponse.captured_amount` | `optional int64` | no | none | `PaymentFlowData.amount_captured` | `Option<i64>` | typed source required | 3 / 4 / 6 | Domain mirror should be removed; proto replacement still needed. |
| `PaymentServiceRefundRequest.payment_amount` | `int64` | no | none | `RefundsData.payment_amount` + `minor_payment_amount` | `i64` + `MinorUnit` | `MinorUnit` + refund/payment currency contract | 3 / 4 / 5 | Proto is raw while refund amount is `Money`; define currency contract or add `payment_amount_money`. |
| `PaymentServiceRefundRequest.refund_amount` | `Money` | no | n/a | `RefundsData.refund_amount` + `minor_refund_amount` + `currency` | `i64` + `MinorUnit` + `Currency` | `MinorUnit` + parent currency | 3 / 5 | Remove domain `i64`; proto is already typed. |
| `PaymentServiceSetupRecurringRequest.amount` | `Money` | no | n/a | `SetupMandateRequestData.amount` + `minor_amount` + `currency` | `Option<i64>` + `Option<MinorUnit>` + `Currency` | `Option<MinorUnit>` + parent currency | 1 / 5 | Remove domain `i64`; proto is already typed. |
| `PaymentServiceSetupRecurringRequest.order_tax_amount` | `optional int64` | no | none | `SetupRecurringRequest.order_tax_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` or proto `Money` replacement | 4 / 5 | Proto remains raw; domain wrapper converts at the boundary. |
| `PaymentServiceSetupRecurringRequest.shipping_cost` | `optional int64` | no | none | `SetupRecurringRequest.shipping_cost` -> `SetupMandateRequestData.shipping_cost` | `Option<MinorUnit>` | `Option<MinorUnit>` or proto `Money` replacement | 4 / 5 | Proto remains raw; setup-recurring conversion now maps shipping from shipping. |
| `PaymentServiceSetupRecurringResponse.captured_amount` | `optional int64` | no | none | `PaymentFlowData.amount_captured` | `Option<i64>` | typed source required | 3 / 4 / 6 | Same captured-amount response problem. |
| `RecurringPaymentServiceChargeRequest.amount` | `Money` | no | n/a | `RepeatPaymentData.amount` + `minor_amount` + `currency` | `i64` + `MinorUnit` + `Currency` | `MinorUnit` + parent currency, with integrity design | 1 / 7 | Domain `i64` interacts with serialized integrity object. |
| `RecurringPaymentServiceChargeRequest.original_payment_authorized_amount` | `optional Money` | no | n/a | `RecurringMandatePaymentData.original_payment_authorized_amount` | `Option<Money>` | `Option<Money>` | 5 | Already typed. |
| `RecurringPaymentServiceChargeRequest.shipping_cost` | `optional int64` | no | none | `RepeatPaymentData.shipping_cost` | `Option<MinorUnit>` | `Option<MinorUnit>` or proto `Money` replacement | 4 / 5 | Proto raw, domain typed. |
| `RecurringPaymentServiceChargeResponse.captured_amount` | `optional int64` | no | none | `PaymentFlowData.amount_captured` | `Option<i64>` | typed source required | 3 / 4 / 6 | Same captured-amount response problem. |
| `OrderDetailsWithAmount.amount` | `int64` | no | none | `payment_address::OrderDetailsWithAmount.amount` | `MinorUnit` | `MinorUnit` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `OrderDetailsWithAmount.total_tax_amount` | `optional int64` | no | none | `payment_address::OrderDetailsWithAmount.total_tax_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `OrderDetailsWithAmount.unit_discount_amount` | `optional int64` | no | none | `payment_address::OrderDetailsWithAmount.unit_discount_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `OrderInfo.discount_amount` | `optional int64` | no | none | `L2L3Data.order_info.discount_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `OrderInfo.shipping_cost` | `optional int64` | no | none | `L2L3Data.order_info.shipping_cost` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `OrderInfo.duty_amount` | `optional int64` | no | none | `L2L3Data.order_info.duty_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `TaxInfo.shipping_amount_tax` | `optional int64` | no | none | `L2L3Data.tax_info.shipping_amount_tax` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |
| `TaxInfo.order_tax_amount` | `optional int64` | no | none | `L2L3Data.tax_info.order_tax_amount` | `Option<MinorUnit>` | `Option<MinorUnit>` with parent/order currency | 4 / 5 | Proto raw, domain typed. |

---

## Bucket 1: Proto Already Has `Money`, Domain Has Duplicate `i64`

These are the safest domain removals. Proto already carries currency and minor amount together, but the domain still keeps an `i64` mirror.

| Proto field | Domain field | Existing typed alternative | Target |
|---|---|---|---|
| `PaymentServiceCaptureRequest.amount_to_capture: Money` | `PaymentsCaptureData.amount_to_capture: i64` | `minor_amount_to_capture: MinorUnit` + `currency` | Remove domain `i64`; use typed field |
| `PaymentServiceSetupRecurringRequest.amount: Money` | `SetupMandateRequestData.amount: Option<i64>` | `minor_amount: Option<MinorUnit>` + `currency` | Remove domain `i64`; use typed field |
| repeat payment request `amount: Money` | `RepeatPaymentData.amount: i64` | `minor_amount: MinorUnit` + `currency` | Remove domain `i64`; keep integrity object separate |

Implementation plan:

1. Confirm the typed alternative is populated in all constructors.
2. Move connector readers to the typed field.
3. Use the domain struct currency when connector conversion requires currency.
4. Remove the raw domain `i64`.
5. Keep proto unchanged.

Do not convert `MinorUnit` back to raw `i64` in connectors.

---

## Bucket 2: Proto Has Deprecated `int64` And Replacement `Money`

These fields are compatibility-only. Domain should use `Money`; deprecated proto fields may remain only at the boundary until the version gate.

| Proto message | Deprecated field | Replacement | Current domain mapping | Target |
|---|---|---|---|---|
| `MandateAmountData` | `amount: int64`, `currency: Currency` | `amount_money: Money` | `mandates::MandateAmountData.amount: Money` | Keep domain as `Money`; remove fallback after min-HS gate |
| `PaymentServiceAuthorizeResponse` | `authorized_amount: int64` | `authorized_money: Money` | currently derived from authorized minor amount | Prefer typed domain source; dual-write only at boundary |

Implementation plan:

1. Keep fallback reads for deprecated fields only while old callers can still send deprecated-only payloads.
2. Keep dual-write only in proto response converters if old callers still read deprecated fields.
3. Do not expose deprecated proto fields as domain fields.
4. After the minimum supported Hyperswitch version is pinned past the migration, remove fallback reads and dual-writes.
5. Later proto phase reserves or removes the deprecated fields.

---

## Bucket 3: Domain `i64` Mirrors With `MinorUnit` Alternatives

These fields can be removed from domain first. Proto may still have raw `int64` fields temporarily, but the domain should not.

| Domain field | Existing typed alternative | Currency source | Target |
|---|---|---|---|
| `PaymentFlowData.amount_captured: Option<i64>` | `minor_amount_captured: Option<MinorUnit>` | flow/request/payment context | Remove domain `i64`; proto converter writes compatibility field from typed source |
| `WebhookDetailsResponse.amount_captured: Option<i64>` | `minor_amount_captured: Option<MinorUnit>` | webhook/resource context may be incomplete | Remove domain `i64`; classify currency need before adding `Money` |
| `RefundsData.payment_amount: i64` | `minor_payment_amount: MinorUnit` | `RefundsData.currency` | Remove domain `i64` |
| `RefundsData.refund_amount: i64` | `minor_refund_amount: MinorUnit` | `RefundsData.currency` | Remove domain `i64` |
| `PaymentsCancelData.amount: Option<i64>` | `minor_amount: Option<MinorUnit>` | `currency: Option<Currency>` | Delete whole struct if still dead |

Implementation plan:

1. Verify every writer co-populates the typed alternative.
2. Move readers to the typed alternative.
3. For proto response compatibility, derive raw proto `int64` only inside `domain_types/src/types.rs`.
4. Delete domain raw fields.
5. Run a grep sweep to ensure no connector reads or writes the removed domain fields.

This bucket should not introduce new helper APIs that convert `MinorUnit` to `i64` for connector use.

---

## Bucket 4: Raw Proto `int64` Amounts With No `Money` Replacement

These cannot be fully removed until proto gets a typed replacement. Domain can still be cleaned where a typed domain field exists.

Examples from `payment.proto`:

| Proto field | Current issue | Direction |
|---|---|---|
| `PaymentServiceAuthorizeResponse.captured_amount` | raw proto `int64`, not marked deprecated | Add replacement `Money` or define parent-currency contract before deprecation |
| `PaymentServiceAuthorizeResponse.capturable_amount` | raw proto `int64`, not marked deprecated | Add replacement `Money` or define parent-currency contract before deprecation |
| `PaymentServiceGetResponse.captured_amount` | raw proto `int64`, not marked deprecated | Add replacement before proto removal |
| `PaymentServiceCaptureResponse.captured_amount` | raw proto `int64`, not marked deprecated | Add replacement before proto removal |
| `PaymentServiceRefundRequest.payment_amount` | raw proto `int64`, while `refund_amount` is `Money` | Add `payment_amount_money` or rely on refund currency by explicit contract |
| tax/shipping/order detail `int64` fields | often parent-currency amounts | Decide `Money` replacement vs parent-currency contract |
| wallet balance / application fee fields | context-specific currency | Needs per-message design |

Implementation plan:

1. Do not remove these proto fields in the domain cleanup PRs.
2. For each field, decide whether the replacement should be `Money`.
3. Add replacement proto fields where currency matters or could differ.
4. Mark raw `int64` as deprecated only after replacement exists.
5. Migrate callers.
6. Reserve/remove raw fields in the later proto cleanup phase.

---

## Bucket 5: Parent-Currency Domain Amounts

Some domain amounts do not need to carry currency because the parent request already has a reliable currency.

Examples:

- capture amount on `PaymentsCaptureData`
- refund amount and original payment amount on `RefundsData`
- setup mandate amount on `SetupMandateRequestData`
- order tax and shipping cost when the request currency is guaranteed

Target domain representation:

- Use `MinorUnit` or `Option<MinorUnit>`.
- Keep currency once on the parent struct.
- Convert to connector-specific amount types using the parent currency.

Use `Money` instead only when:

- the amount can have a currency different from the parent;
- the amount travels independently of the parent request;
- storing currency with the amount avoids an ambiguous or unsafe lookup.

---

## Bucket 6: Amounts Without Reliable Currency

These need a design decision before choosing `Money` or `MinorUnit`.

Examples:

- webhook captured amount;
- payment-flow captured/capturable amounts when the response path does not carry currency;
- wallet balance and stored-value payload fields;
- any amount extracted from connector webhooks without a resolved payment context.

Decision process:

1. Identify consumers of the amount.
2. If consumers only compare or forward a minor amount with already-known context, use `MinorUnit`.
3. If consumers need currency or the amount may outlive its context, use `Money`.
4. If currency is missing but required, add explicit context before migrating.
5. Do not infer currency from nearby fields unless the flow contract guarantees it.

---

## Bucket 7: Integrity And Serialized Semantic Fields

These are not mechanical migrations. They can change runtime comparison semantics or serialized shapes.

| Field | Problem | Direction |
|---|---|---|
| `RepeatPaymentIntegrityObject.amount: i64` | serialized and compared at runtime | Design protocol migration separately |
| TSYS repeat-payment fallback to request raw amount | depends on `RepeatPaymentData.amount` | Keep until integrity shape changes |
| any request/response integrity object with serialized amount shape | external or persisted comparison may change | migrate only with tests and compatibility story |

Implementation plan:

1. Decide whether integrity compares `MinorUnit` or `Money`.
2. If moving to `Money`, update serialized shape intentionally.
3. Update integrity generation, comparison, and tests together.
4. Keep old compatibility shape isolated if old events/responses can still be compared.

---

## Execution Order

| Phase | Scope | Reason |
|---|---|---|
| 0 | Build proto-to-domain inventory table | Prevent accidental removals and clarify replacements |
| 1 | Bucket 1 request fields | Smallest clean domain removals where proto already has `Money` |
| 2 | Bucket 3 easy mirrors | Remove domain-only `i64` mirrors with populated typed alternatives |
| 3 | Bucket 2 deprecated proto compatibility | Keep or remove fallback/dual-write based on min-HS version |
| 4 | Bucket 4 proto replacement additions | Add `Money` replacements for raw proto `int64` fields |
| 5 | Bucket 6 currency-context decisions | Avoid unsafe `Money` conversions |
| 6 | Bucket 7 integrity migration | Separate semantic change |

---

## Verification Per PR

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- Grep for domain raw amount fields:
  - `pub .*amount.*:.*i64`
  - `pub .*captured.*:.*i64`
  - `pub .*tax.*:.*i64`
  - `pub .*cost.*:.*i64`
- Grep for connector usage of removed fields.
- Grep for `legacy_amount_as_i64`; connector code should not gain new usage.
- For proto changes, verify generated SDK/server tests and compatibility fixtures.

---

## Current Raw Domain Amount Fields

Current scan of `crates/types-traits/domain_types/src` finds these raw domain amount fields:

| Field | Existing typed alternative | Bucket |
|---|---|---|
| `router_request_types::PaymentsCancelData.amount: Option<i64>` | `minor_amount: Option<MinorUnit>` | 3 |
| `router_request_types::RepeatPaymentIntegrityObject.amount: i64` | none | 7 |
| `connector_types::PaymentFlowData.amount_captured: Option<i64>` | `minor_amount_captured: Option<MinorUnit>` | 3 / 6 |
| `connector_types::WebhookDetailsResponse.amount_captured: Option<i64>` | `minor_amount_captured: Option<MinorUnit>` | 3 / 6 |
| `connector_types::RefundsData.payment_amount: i64` | `minor_payment_amount: MinorUnit` | 3 / 5 |
| `connector_types::RefundsData.refund_amount: i64` | `minor_refund_amount: MinorUnit` | 3 / 5 |
| `connector_types::PaymentsCaptureData.amount_to_capture: i64` | `minor_amount_to_capture: MinorUnit` | 1 / 5 |
| `connector_types::SetupMandateRequestData.amount: Option<i64>` | `minor_amount: Option<MinorUnit>` | 1 / 5 |
| `connector_types::RepeatPaymentData.amount: i64` | `minor_amount: MinorUnit` | 1 / 7 interaction |
| `types::AuthorizationRequest.order_tax_amount: Option<i64>` | none | 4 / 5 |
| `types::AuthorizationRequest.shipping_cost: Option<i64>` | none | 4 / 5 |
| `types::SetupRecurringRequest.order_tax_amount: Option<i64>` | none | 4 / bug fix |
| `types::SetupRecurringRequest.shipping_cost: Option<i64>` | none | 4 / bug fix |

---

## Current Deprecated Proto Amount Fields

From `crates/types-traits/grpc-api-types/proto/payment.proto`:

| Proto field | Replacement | Domain state |
|---|---|---|
| `MandateAmountData.amount: int64` | `amount_money: Money` | domain already uses `Money` |
| `MandateAmountData.currency: Currency` | `amount_money.currency` | domain already uses `Money` |
| `PaymentServiceAuthorizeResponse.authorized_amount: int64` | `authorized_money: Money` | should remain boundary-only compatibility |

Other deprecated proto fields found in the scan are not amount fields.
