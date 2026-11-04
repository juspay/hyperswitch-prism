# Integrity Check Pattern

## Overview

Integrity checks compare the original UCS request invariants with a response-side
integrity object populated by the connector transformer. They are not automatic
just because a response contains an amount or id. The response transformer must
write the comparable values back onto the cloned request:

```rust
Ok(Self {
    response: Ok(...),
    request: PaymentsCaptureData {
        integrity_object: Some(CaptureIntegrityObject {
            amount_to_capture: response_amount,
            currency: router_data.request.currency,
        }),
        ..router_data.request.clone()
    },
    ..router_data.clone()
})
```

If `get_response_integrity_object()` returns `None`, the framework skips the
check. Leaving `integrity_object: None` on a flow whose response has comparable
fields silently disables tamper detection.

Transit (`tsys_transit`) is the current payment-flow exemplar:

- `connectors/tsys_transit/transformers.rs` populates `AuthoriseIntegrityObject`,
  `PaymentSynIntegrityObject`, `CaptureIntegrityObject`, `RefundIntegrityObject`,
  `RefundSyncIntegrityObject`, `PaymentVoidIntegrityObject`, and
  `PaymentVoidPostCaptureIntegrityObject`.
- It parses echoed amounts when Transit returns them, and falls back to request
  values only where Transit genuinely does not echo the field. Those fallbacks
  carry comments explaining the connector contract.

## Framework Shape

- Request data exposes two values through
  `interfaces/src/integrity.rs`:
  - `get_request_integrity_object()` derives invariants from the original
    request.
  - `get_response_integrity_object()` reads the optional response-side object
    from the same request struct.
- `FlowIntegrity::compare()` compares those two objects field by field.
- `ConnectorError::IntegrityCheckFailed` and status mapping to integrity failure
  are framework concerns. Do not manually manufacture an integrity failure
  status in the connector transformer.

## Flow Objects

Use the object already defined for the request type in
`crates/types-traits/domain_types/src/router_request_types.rs`.

| Flow | Request type | Integrity object | Fields compared |
| --- | --- | --- | --- |
| Authorize | `PaymentsAuthorizeData<T>` | `AuthoriseIntegrityObject` | `amount`, `currency` |
| PSync | `PaymentsSyncData` | `PaymentSynIntegrityObject` | `amount`, `currency` |
| Capture | `PaymentsCaptureData` | `CaptureIntegrityObject` | `amount_to_capture`, `currency` |
| Void | `PaymentVoidData` | `PaymentVoidIntegrityObject` | `connector_transaction_id` |
| VoidPC | `PaymentsCancelPostCaptureData` | `PaymentVoidPostCaptureIntegrityObject` | `connector_transaction_id` |
| Refund | `RefundsData` | `RefundIntegrityObject` | `refund_amount`, `currency` |
| RSync | `RefundSyncData` | `RefundSyncIntegrityObject` | `connector_transaction_id`, `connector_refund_id` |
| VoidPostRefund | `RefundVoidPostRefundData` | `RefundSyncIntegrityObject` | empty `connector_transaction_id`, `connector_refund_id` |
| SetupMandate | `SetupMandateRequestData<T>` | `SetupMandateIntegrityObject` | optional `amount`, `currency` |
| RepeatPayment | `RepeatPaymentData<T>` | `RepeatPaymentIntegrityObject` | `amount`, `currency`, `mandate_reference` |
| PaymentMethodToken | `PaymentMethodTokenizationData<T>` | `PaymentMethodTokenIntegrityObject` | `amount`, `currency` |
| PreAuthenticate / Authenticate / PostAuthenticate | payment-auth request data | matching auth integrity object | `amount`, `currency` |

Some categories intentionally use no-op objects, such as
`ClientAuthenticationTokenIntegrityObject`, `GetPaymentMethodIntegrityObject`,
and the FRM integrity objects. Do not invent fields for those categories; only
populate a response-side object when the request type actually stores one.

## Implementation Rules

1. Populate integrity objects in successful response `TryFrom` impls, where both
   `router_data.request` and the parsed connector response are available.
2. Prefer connector-echoed values over request values. For example, if the
   capture response echoes `transactionAmount`, parse it and use that amount.
3. Convert echoed amounts back into framework units with the connector's amount
   converter before placing them in the integrity object.
4. Use request values only when the connector does not return a comparable field
   by contract. Add a short comment that names the missing connector field or
   behavior.
5. Do not compare approximations. If the connector returns a net amount, fee,
   authorized amount, or display amount that is not the same invariant the
   framework checks, do not stuff it into the integrity object.
6. For sync flows, be careful about the lookup key. If a connector response
   returns the id of the transaction that was queried rather than the refund id,
   mirror the request-side ids exactly instead of manufacturing a mismatch.

## Transit Examples

### Authorize / PSync

Transit sometimes returns a processed amount. When it does, parse it back into
`MinorUnit`; when it does not, Transit has not echoed currency and may not echo
an auth-only amount, so the transformer falls back to the request values with a
comment:

```rust
request: PaymentsAuthorizeData {
    integrity_object: Some(AuthoriseIntegrityObject {
        amount: minor_amount_captured
            .or(minor_amount_capturable)
            .unwrap_or(router_data.request.amount),
        currency: router_data.request.currency,
    }),
    ..router_data.request.clone()
},
```

For PSync, Transit uses an ambiguous amount string. Its helper treats a decimal
point as major units and a plain integer as minor units before constructing
`PaymentSynIntegrityObject`.

### Capture

Transit capture responses may echo `transactionAmount`. Use the echoed capture
amount when present; fall back to `minor_amount_to_capture` only when Transit
omits it:

```rust
request: PaymentsCaptureData {
    integrity_object: Some(CaptureIntegrityObject {
        amount_to_capture: minor_amount_captured
            .unwrap_or(router_data.request.minor_amount_to_capture),
        currency: router_data.request.currency,
    }),
    ..router_data.request.clone()
},
```

### Refund / RSync

Transit refund responses may echo `returnedAmount`. Parse it when present, then
construct `RefundIntegrityObject`. For RSync, preserve the exact request lookup
ids:

```rust
request: RefundSyncData {
    integrity_object: Some(RefundSyncIntegrityObject {
        connector_transaction_id: router_data.request.connector_transaction_id.clone(),
        connector_refund_id: router_data.request.connector_refund_id.clone(),
    }),
    ..router_data.request.clone()
},
```

This avoids a false mismatch when the inquiry was keyed by payment transaction
id because the refund id was empty.

## Review Checklist

- Every response transformer for a flow with an `integrity_object` field sets
  `Some(...)` when the connector returns comparable values.
- Every fallback to request amount, currency, transaction id, refund id, or
  mandate reference has a comment explaining why the connector did not provide a
  comparable response field.
- No response transformer leaves `integrity_object: None` merely because the
  scaffold started that way.
- Unit tests cover at least one success response that populates the integrity
  object and one response where the connector omits a field that intentionally
  falls back to the request.
