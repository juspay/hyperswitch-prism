# InterPayments surcharge refunds: phase 2

## Scope and flow

Prism exposes two new `SurchargeService` RPCs: `SurchargeRefundInit` and
`SurchargeRefundComplete`. A caller first sends the original surcharge
transaction ID and the amount of the payment being refunded to Init. Init
returns the surcharge refund amount, the original fee, fees previously
refunded, and a **new refund transaction ID**. The caller persists that new ID
and sends it to Complete after the payment refund succeeds. Full and partial
refunds use the same two calls; the amount supplied to each Init determines
the refund portion. An existing `SurchargeRefundSucceeded` event maps to the
older `/ch/refund` endpoint and remains a separate operation.

## API reference

The InterPayments examples supplied for this phase define these requests and
responses. Authentication is the existing InterPayments Bearer API key; the
example token is intentionally omitted here.

| Prism RPC | InterPayments request | InterPayments response |
| --- | --- | --- |
| `SurchargeService.SurchargeRefundInit` | `POST /ch/refund/init` with `{ "sTxId": "<original surcharge ID>", "amount": 500 }` | `{ "refund": 15, "originalTransactionFee": 30, "previouslyRefundedTransactionFees": 0, "sTxId": "<refund ID>" }` |
| `SurchargeService.SurchargeRefundComplete` | `POST /ch/refund/complete` with `{ "sTxId": "<refund ID>" }` | `{ "sTxId": "<refund ID>" }` |

InterPayments also returns two error shapes for these calls. A transaction
state error has `reasonCode`, `message`, and `reason` (plus optional
`reasonCodes`, `reasonCodeIds`, `code`, and `seeAlso`). Prism uses `reasonCode`
as the stable connector error code. An Init validation error can instead be
`{ "error": { "amount": ["Amounts are required to be >= 0"] } }`.
Prism maps this to `VALIDATION_ERROR` and includes each field and message in
the connector error text.

The configured base URL already includes `/v1` in Prism. The examples do not
establish whether the vendor's numeric `amount` and fee fields are major or
minor currency units. The implementation will use the connector's existing
major-unit amount converter and expose `Money` in minor units over gRPC;
this unit assumption needs validation against InterPayments documentation or
a test account before production use. A positive refund amount and a currency
are required. Complete validates that the returned ID matches the requested ID.
Under that assumption, an InterPayments `amount` of `500` USD is sent from
Prism as `Money { minor_amount: 50000, currency: USD }`, and a returned
`refund` of `15` becomes `Money { minor_amount: 1500, currency: USD }`.

## Data shapes

* Init request: `merchant_surcharge_refund_id` (optional trace reference),
  `connector_surcharge_id` (original `sTxId`), `amount` (`Money`).
* Init response: echoed merchant reference, `connector_surcharge_refund_id`
  (new `sTxId`), `refund_amount`, `original_transaction_fee`,
  `previously_refunded_transaction_fees` (`Money` in the requested currency),
  `status_code`, `error`.
* Complete request: optional merchant reference and required
  `connector_surcharge_refund_id` from Init.
* Complete response: echoed merchant reference and refund ID, `status_code`,
  `error`.

Domain request and response structs mirror these fields with `MinorUnit` and
`Currency` for amounts. The connector transformer has separate wire structs
with InterPayments camel-case JSON names. Existing connector error mapping is
used for non-2xx responses, and the gRPC response carries connector errors in
`error`, as Calculate already does.

## Prism changes

1. Add messages to `surcharge.proto` and RPCs to `services.proto`.
2. Add flow markers, event names, connector traits, domain data, proto
   conversions, and response generation.
3. Route both methods through the surcharge gRPC service and InterPayments
   connector with Bearer authentication and JSON requests.
4. Extend the FFI and generated SDK flow surfaces so the RPCs are available
   through Prism clients, then verify format, compilation, and focused tests.

Hyperswitch orchestration and persistence are deferred to the later HS phase.
