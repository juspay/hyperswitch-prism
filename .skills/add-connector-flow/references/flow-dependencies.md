# Flow Dependencies Reference

All flow names in this document are the **marker structs** declared in
`crates/types-traits/domain_types/src/connector_flow.rs` -- the same spelling
`create_all_prerequisites!` and `macro_connector_implementation!` take. Verify any name
before you use it:

```bash
rg -w '<Name>' crates/types-traits/domain_types/src/connector_flow.rs
```

`connector_flow.rs` also declares a `FlowName` enum, and its variant set is not the marker
set. Two `FlowName` variants have no marker at all: `IncomingWebhook` and `Dsync` (one
capital -- not `DSync`). Conversely `Accept`, `PSync`, `RSync`, `VoidPC` and
`VerifyWebhookSource` are markers with no `FlowName` variant of the same spelling -- their
`FlowName` counterparts are spelled `AcceptDispute`, `Psync`, `Rsync` and `VoidPc`
(`VerifyWebhookSource` has none). The macros take the **marker** spelling; do not paste a
`FlowName` variant into one. There is no `CreateAccessToken`, `CreateSessionToken`, `PaymentAccessToken` or
`PaymentSessionToken` anywhere in `crates/`; the token flows are `ServerAuthenticationToken`,
`ServerSessionAuthenticationToken` and `ClientAuthenticationToken`.

## Dependency Graph

```
   [No prerequisite]
   ServerAuthenticationToken, ServerSessionAuthenticationToken,
   ClientAuthenticationToken, CreateOrder, CreateConnectorCustomer,
   Accept, SubmitEvidence, DefendDispute

   CreateConnectorCustomer ──> PaymentMethodToken
   CreateConnectorCustomer ──> SetupMandate ──> RepeatPayment
                                    └────────> MandateRevoke

   [Main payment chain]

                     Authorize
        ┌───────┬────────┴────────┬──────────────────────┐
      PSync   Void(manual)   Capture(manual)   IncrementalAuthorization(manual)
                                  │
                                VoidPC

                     Authorize ──> Refund ──> RSync
                                    ^
                     Capture ───────┘  (only when capture_method = manual)
```

`Refund` has two inbound edges on purpose. See the Refund row below -- collapsing them
into a single "Refund needs Capture" edge is the most common error in this table.

## Dependency Map

| Flow | Prerequisites | Why (evidence) |
|------|---------------|----------------|
| Authorize | (none) | Foundation. |
| PSync | Authorize | `PaymentService_Get/suite_spec.json` threads Authorize's `res.connector_transaction_id` straight into the sync request. |
| Capture | Authorize, **manual capture only** | `PaymentService_Capture/suite_spec.json` depends on Authorize scenario `no3ds_manual_capture_credit_card`. An auto-capture connector has no Capture step to add. |
| Void | Authorize, **manual capture only** | Void cancels an *uncaptured* authorization -- the exact opposite of Refund. `PaymentService_Void/suite_spec.json` depends on Authorize scenario `no3ds_manual_capture_credit_card`. |
| VoidPC | Authorize + Capture | Reverses an *already-captured* payment. `PaymentService_Reverse/suite_spec.json` is the only suite in the repo that names `PaymentService/Capture` as a dependency. |
| Refund | Authorize **always**; Capture **only if the connector runs manual capture** | Conditional -- see below. |
| RSync | Refund | `RefundService_Get/suite_spec.json` maps `res.connector_refund_id` out of the `PaymentService/Refund` suite; RSync consumes the id Refund produces. |
| SetupMandate | (none) -- `CreateConnectorCustomer` in practice | **Not Authorize.** `PaymentService_SetupRecurring/suite_spec.json` `depends_on` is `[MerchantAuthenticationService/CreateServerAuthenticationToken, CustomerService/Create]`. SetupMandate is a zero/low-amount card-on-file setup: a *sibling* of Authorize, not a successor. |
| RepeatPayment | SetupMandate | `RecurringPaymentService_Charge/suite_spec.json` maps `res.mandate_reference.connector_mandate_id.connector_mandate_id` from `PaymentService/SetupRecurring`. |
| MandateRevoke | SetupMandate | `RecurringPaymentService_Revoke/suite_spec.json` `depends_on` is `[CustomerService/Create, PaymentService/SetupRecurring]`. |
| IncrementalAuthorization | Authorize, **manual capture only** | `PaymentService_IncrementalAuthorization/suite_spec.json` depends on Authorize scenario `no3ds_manual_capture_incremental_auth`. |
| IncomingWebhook | (none proven) | `EventService_HandleEvent/suite_spec.json` `depends_on` is `[]`, and `IncomingWebhook` is a plain trait (`interfaces/src/connector_types.rs`), not a `ConnectorIntegrationV2` flow -- nothing enforces an order at compile time. Sequence it after Authorize anyway: a webhook carries status for a payment that must already exist. |
| ServerAuthenticationToken | (none) | `MerchantAuthenticationService_CreateServerAuthenticationToken/suite_spec.json` `depends_on` is `[]`. |
| ServerSessionAuthenticationToken | (none) | `depends_on` is `[]`. **Does not require ServerAuthenticationToken:** the enabling gates are disjoint in practice -- the connectors overriding `should_do_session_token` and those overriding `should_do_access_token` are two non-overlapping sets. |
| ClientAuthenticationToken | (none) | `depends_on` is `[]`. Most connectors doing real `ClientAuthenticationTokenRequestData` work never touch access tokens. |
| CreateOrder | (none) | `PaymentService_CreateOrder/suite_spec.json` `depends_on` is `[]`. |
| CreateConnectorCustomer | (none) | `CustomerService_Create/suite_spec.json` `depends_on` is `[]`. |
| PaymentMethodToken | CreateConnectorCustomer | **Not Authorize -- the arrow points the other way.** `PaymentMethodService_Tokenize/suite_spec.json` `depends_on` is `["CustomerService/Create"]`, while `PaymentService_Capture`, `_Get`, `_Refund` and `_Void` all list `PaymentMethodService/Tokenize` as *their* dependency. Structurally, `PaymentMethodTokenizationData` has no `connector_transaction_id` field, so it cannot consume an Authorize result; and `crates/internal/composite-service/src/payment_methods.rs` tokenizes before it authorizes. |
| Accept | (none, unproven) | No dispute `suite_spec.json` exists. `AcceptDisputeData` keys off a dispute id, not a payment id. |
| SubmitEvidence | (none, unproven) | No dispute `suite_spec.json` exists. |
| DefendDispute | (none, unproven) | No dispute `suite_spec.json` exists. |

**Unproven rows.** `suite_spec.json` files live under
`crates/internal/integration-tests/src/global_suites/`. Only flows with one there have a
*proven* dependency edge. The dispute flows have none, so their "(none)" is an inference
from request-type shape and rpc placement, not a fact. Do not harden it.

## The Refund Prerequisite (read this before editing the table)

The honest answer is conditional, and three documents in this repo have each picked a
different half of it. The rule:

> **Refund's code-level prerequisite is Authorize. Its *semantic* prerequisite is a
> captured payment.** Under `capture_method = automatic`, Authorize alone satisfies both.
> Under `capture_method = manual`, Capture must run first.

Evidence for the code-level half:

- `RefundsData.connector_transaction_id` (`domain_types/src/connector_types.rs`) is a
  **non-`Option` `String`** -- and it holds the *Authorize* id, not a capture id.
- `RefundsData` also carries `capture_method: Option<CaptureMethod>`, which only exists
  because refund behaviour varies with the capture model.
- Four independent connectors build the refund endpoint from the Authorize id and never
  from a capture id: `checkout.rs` (`payments/{connector_tx_id}/refunds`), `razorpay.rs`
  (`v1/payments/{connector_transaction_id}/refund`), `cybersource.rs`
  (`pts/v2/payments/{connector_payment_id}/refunds`) and `adyen.rs`
  (`{ADYEN_API_VERSION}/payments/{connector_payment_id}/refunds`).

Evidence for the semantic half:

- `PaymentService_Refund/suite_spec.json` depends on Authorize scenario
  **`no3ds_auto_capture_credit_card`**, with **no Capture suite anywhere in the chain** --
  whereas `PaymentService_Capture` depends on `no3ds_manual_capture_credit_card`.

So when validating a Refund request: require Authorize unconditionally, and require
Capture **only** if the connector's tech spec says it is manual-capture.

## Resolution Algorithm

Given `requested_flows`, `existing_flows` and the connector's `capture_method`, determine
implementation order:

```
0. EXPAND conditional prerequisites:
   - Refund gains a Capture prerequisite only when capture_method = manual.
   - Capture, Void and IncrementalAuthorization are meaningful only under manual capture;
     under automatic capture, flag them to the user rather than silently ordering them.

1. VALIDATE: For each requested flow, check every prerequisite is either
   in existing_flows or in requested_flows. If not, report error and stop.
   Prerequisites marked "(unproven)" are advisory -- warn, do not block.

2. SORT (topological): Repeat until remaining is empty:
   a. Find all flows in remaining whose prerequisites are all satisfied
      (in existing_flows or already in the ordered output list).
   b. If none found, report circular dependency error.
   c. Move those flows from remaining to the ordered output list.

3. Return the ordered list.
```

## Example Resolutions

**Adding [RSync, Refund], existing = [Authorize, PSync, Capture], manual capture:**
- Refund: needs Authorize (existing); manual capture, so also Capture (existing) -> OK
- RSync: needs Refund (in requested) -> OK
- Order: `[Refund, RSync]`

**Adding [RSync, Refund], existing = [Authorize, PSync], automatic capture:**
- Refund: needs Authorize (existing); auto capture, so no Capture edge -> OK
- RSync: needs Refund (in requested) -> OK
- Order: `[Refund, RSync]` -- **not** an error. Do not demand Capture from an
  auto-capture connector.

**Adding [Refund, Capture], existing = [Authorize], manual capture:**
- Capture: needs Authorize (existing) -> OK
- Refund: needs Authorize (existing) + Capture (in requested) -> OK
- Order: `[Capture, Refund]`

**Adding [Refund], existing = [Authorize, PSync], manual capture (ERROR):**
- Refund: manual capture, so needs Capture -> NOT in existing, NOT in requested -> ERROR
- Message: "Cannot add Refund to a manual-capture connector: prerequisite Capture is not
  implemented. Add Capture in the same batch, or confirm the connector is auto-capture."

**Adding [RepeatPayment], existing = [Authorize, PSync] (ERROR):**
- RepeatPayment: needs SetupMandate -> missing -> ERROR
- Note SetupMandate itself does **not** need Authorize, so the fix is
  `[SetupMandate, RepeatPayment]`, not a longer chain.

## Detecting Existing Flows

Inspect the connector `.rs` file. The two **reliable** indicators, both of which must be
present for a flow to count as implemented:

1. **`create_all_prerequisites!` api array** -- an `(flow: <Marker>, ...)` entry, e.g.
   `(flow: Capture, request_body: ..., response_body: ..., router_data: RouterDataV2<Capture, ...>)`.
2. **`macro_connector_implementation!` block** -- `flow_name: <Marker>`, e.g.
   `flow_name: ServerAuthenticationToken`.

A flow present in only one of the two is not implemented.

**Trait impls are a weak signal -- do not use them to detect support.** `ConnectorServiceTrait`
requires the whole flow family as supertraits, and `macro_connector_flow_status_impls!`
(`crates/integrations/connector-integration/src/connectors/macros.rs` ~:1827, via
`expand_flow_status_impl!` ~:1945) emits, for every flow it names, BOTH the empty marker
impl (`impl<T: ...> ::interfaces::connector_types::PaymentCapture for $c<$g> {}`) and a stub
`ConnectorIntegrationV2` impl whose `get_url` returns
`IntegrationError::connector_flow_not_implemented(...)`. So nearly every connector carries a
stub for nearly every flow. Seeing `connector_types::RefundV2 for Checkout<T> {}` proves
nothing.

**The negative signal is exact, though.** A marker name still sitting inside
`macro_connector_flow_status_impls!`'s `not_implemented: [...]` or `not_supported: [...]`
list is definitively NOT implemented -- the flow could not appear in
`macro_connector_implementation!` as well without a conflicting implementation (E0119).
Grep that invocation first; it is faster than reading the whole file. `macro_connector_payout_implementation!`
(~:1448) does the same job for the `PayoutXxxV2` family, and
`macro_connector_local_flow_implementation!` (~:2425) covers flows with no outbound HTTP call.

For the gated pre-auth flows, the real signal is an **override of the `ValidationTrait`
gate** (declared in `crates/types-traits/interfaces/src/connector_types.rs`):

| Flow | Gate |
|------|------|
| ServerAuthenticationToken | `should_do_access_token` |
| ServerSessionAuthenticationToken | `should_do_session_token` |
| PaymentMethodToken | `should_do_payment_method_token` |
| CreateOrder | `should_do_order_create` |
| CreateConnectorCustomer | `should_create_connector_customer` |

...or a real `ForeignTryFrom` in that connector's `transformers.rs` for the flow's request
data type (e.g. `ServerAuthenticationTokenRequestData`).

## Handling Missing Prerequisites

1. **Inform the user** which prerequisite is missing and which flow requires it.
2. **Suggest** adding the prerequisite to the requested set, or implementing it first.
3. **Do not proceed** with the dependent flow until prerequisites are resolved.
4. **Independent flows can proceed.** If requesting [Void, Refund] on a manual-capture
   connector and Capture is missing, Void can still be implemented (it needs only
   Authorize). Only Refund is blocked.
5. **State the condition, not a verdict.** When a prerequisite is capture-model dependent,
   say so: "Refund needs Capture *because this connector is manual-capture*." That one
   clause stops the next person re-deriving it.
