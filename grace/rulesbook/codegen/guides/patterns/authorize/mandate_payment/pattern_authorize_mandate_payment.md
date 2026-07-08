# MandatePayment Authorize Flow Pattern
-

## Overview

`MandatePayment` is a **`PaymentMethodData` variant** used inside an Authorize
request when the merchant is paying using an existing mandate reference
(as opposed to `SetupMandate`-the-flow, which *creates* the mandate, or
`RepeatPayment`-the-flow, which *charges* one). In other words:

- **SetupMandate (flow)**: registers a customer payment instrument with the
  connector and returns a `connector_mandate_id`. See the flow marker
  `domain_types::connector_flow::SetupMandate` — declared at
  `crates/types-traits/domain_types/src/connector_flow.rs:23`.
- **RepeatPayment (flow)**: a dedicated MIT flow carrying the mandate reference
  on `RepeatPaymentData.mandate_reference`
  (`crates/types-traits/domain_types/src/connector_types.rs:2532`). Uses the
  flow marker `RepeatPayment` declared at
  `crates/types-traits/domain_types/src/connector_flow.rs:26`.
- **MandatePayment (PM)**: the `PaymentMethodData::MandatePayment` unit variant
  at `crates/types-traits/domain_types/src/payment_method_data.rs:261`,
  embedded inside a normal `Authorize` request via `PaymentsAuthorizeData<T>`.
  It signals "use the mandate_id carried elsewhere on the request; the
  cardholder data is not provided in this PM payload". It is the
  **sign of a merchant-initiated transaction expressed through the Authorize
  flow**, where the connector either (a) rejects it and forces the caller
  to use `RepeatPayment` (e.g. Worldpay at
  `crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:144-148`),
  or (b) dispatches to a mandate-specific request builder (e.g. ACI, Braintree,
  Cybersource, Zift, Revolv3).

Because `MandatePayment` is a **unit variant** (no associated data),
everything the connector needs to authorize a mandated charge must be
read from **surrounding fields** on `PaymentsAuthorizeData<T>` or from
`RepeatPaymentData<T>` when the caller chose the RepeatPayment flow. The
canonical mandate reference field is `PaymentsAuthorizeData::mandate_id:
Option<MandateIds>` at
`crates/types-traits/domain_types/src/connector_types.rs:1110`, with the
`MandateIds` type itself declared at
`crates/types-traits/domain_types/src/connector_types.rs:344-348`.

### Key Characteristics

| Aspect | Value |
|--------|-------|
| Enum position | `PaymentMethodData::MandatePayment` (unit variant) at `crates/types-traits/domain_types/src/payment_method_data.rs:261` |
| Inner fields | None — unit variant |
| Where mandate reference comes from | `PaymentsAuthorizeData.mandate_id: Option<MandateIds>` at `connector_types.rs:1110`, or `RepeatPaymentData.mandate_reference: MandateReferenceId` at `connector_types.rs:2532` |
| Canonical mandate type | `MandateIds` at `connector_types.rs:344-348` |
| Related `PaymentFlowData` field | `recurring_mandate_payment_data: Option<RecurringMandatePaymentData>` at `connector_types.rs:459` |
| Interaction model | Merchant-initiated (MIT); no cardholder data in the PM payload |
| Preferred flow for new code | `RepeatPayment` flow (see `pattern_repeat_payment_flow.md`) |
| Fallback when Authorize is the only path | Dispatch on `PaymentMethodData::MandatePayment` and build a mandate-specific request (see ACI, Braintree examples below) |

---

## Variant Enumeration

`MandatePayment` is a **unit variant** of the `PaymentMethodData<T>` enum. The
variant has no associated data, so the "Data Shape" column below documents the
absence of fields explicitly. The table lists exactly one row — the enum's
sole MandatePayment variant — to satisfy the Spec §9 requirement that every
variant of the relevant PM be enumerated.

| Variant | Data Shape | Citation | Used By (connectors) |
|---------|------------|----------|----------------------|
| `MandatePayment` | **unit variant** (no fields). The mandate reference is read from the surrounding `PaymentsAuthorizeData::mandate_id` or `RepeatPaymentData::mandate_reference`. | `crates/types-traits/domain_types/src/payment_method_data.rs:261` | ACI (`aci/transformers.rs:729-737`), Braintree (`braintree/transformers.rs:2785-2797`), Cybersource (`cybersource/transformers.rs:4295-4304`), Revolv3 (`revolv3/transformers.rs:1133`), Zift (`zift/transformers.rs:736-772`), Novalnet (`novalnet/transformers.rs:2322-2324`), Checkout (`checkout/transformers.rs:880-884`), Multisafepay (`multisafepay/transformers.rs:80`, `multisafepay/transformers.rs:325`), Worldpay (rejects → `RepeatPayment`, `worldpay/transformers.rs:144-148`) |

> **Variant-count sanity check.** The `PaymentMethodData<T>` enum at the pinned
> SHA has twenty variants declared between lines 248-271 of
> `payment_method_data.rs`. This PM pattern only owns one of them
> (`MandatePayment` at line 261); the other nineteen are owned by sibling PM
> patterns under `authorize/<pm>/`. No variant is orphaned.

---

## Architecture Overview

### Flow Type

The Authorize flow marker is `domain_types::connector_flow::Authorize` —
`crates/types-traits/domain_types/src/connector_flow.rs:5`.

MandatePayment is the **payload shape**, not the flow. The router decides
whether to dispatch to Authorize (this pattern) or RepeatPayment (see
`pattern_repeat_payment_flow.md`) based on whether the connector has a
dedicated RepeatPayment implementation. Connectors that implement RepeatPayment
natively (listed in `pattern_repeat_payment_flow.md` §Connectors with Full
Implementation) generally **do not** handle `PaymentMethodData::MandatePayment`
inside Authorize — they match on `RepeatPaymentData.mandate_reference`
instead. See Cybersource's split at
`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:4292-4304`.

### Request Type

`PaymentsAuthorizeData<T>` — declared at
`crates/types-traits/domain_types/src/connector_types.rs:1088-1150`.

Key fields relevant to MandatePayment:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1088
pub struct PaymentsAuthorizeData<T: PaymentMethodDataTypes> {
    pub payment_method_data: PaymentMethodData<T>, // variant we match on
    pub amount: MinorUnit,
    pub currency: Currency,
    pub capture_method: Option<common_enums::CaptureMethod>,
    // Mandates
    pub mandate_id: Option<MandateIds>,            // <-- line 1110: canonical mandate reference
    pub setup_future_usage: Option<common_enums::FutureUsage>,
    pub off_session: Option<bool>,
    pub customer_acceptance: Option<CustomerAcceptance>,
    pub setup_mandate_details: Option<MandateData>,
    // ...
}
```

`PaymentsAuthorizeData::is_mandate_payment()` (declared at
`crates/types-traits/domain_types/src/connector_types.rs:1231-1239`) is the
canonical predicate connectors may call to detect any form of mandate-flavoured
authorize (CIT-with-mandate-setup OR MIT-by-reference).

### Response Type

`PaymentsResponseData` — declared in
`crates/types-traits/domain_types/src/connector_types.rs`.

For MandatePayment inside Authorize, the response is the normal
`PaymentsResponseData::TransactionResponse` shape. The connector **does not**
populate `mandate_reference` again on success — the mandate already exists and
its id came in on the request. `mandate_reference: None` is correct for MIT.

### Resource Common Data

`PaymentFlowData` — declared at
`crates/types-traits/domain_types/src/connector_types.rs:422-464`.

Notably, `PaymentFlowData.recurring_mandate_payment_data:
Option<RecurringMandatePaymentData>` at
`crates/types-traits/domain_types/src/connector_types.rs:459` carries
connector-agnostic recurring context (original authorized amount,
`payment_method_type`, and `mandate_metadata`). The type itself is
`RecurringMandatePaymentData` at
`crates/types-traits/domain_types/src/router_data.rs:3008-3012`. A few
connectors consult this to build the mandate request payload. However, the
**primary** canonical location of the mandate id is the request-side
`PaymentsAuthorizeData.mandate_id`, not `PaymentFlowData`.

### Canonical RouterDataV2 signature

```rust
// PM pattern — Authorize path
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

This matches the canonical signature from the spec (`PATTERN_AUTHORING_SPEC.md`
§7 "Canonical Type Signatures"). Note the four generic parameters — authors
MUST NOT drop any of them.

### Unwrapping the variant

Because `MandatePayment` is unit, unwrapping is a simple match with no
destructured fields:

```rust
match &router_data.request.payment_method_data {
    PaymentMethodData::MandatePayment => {
        // the PM itself has no data; pull the mandate reference from
        // router_data.request.mandate_id (an Option<MandateIds>)
        let mandate_ids = router_data
            .request
            .mandate_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "mandate_id",
                context: Default::default(),
            })?;
        // ... build mandate-specific request
    }
    _ => { /* other PM arms */ }
}
```

This pattern is mirrored in the production connectors enumerated below.

---

## Flow composition

The following diagram distinguishes the three mandate-related constructs and
shows where MandatePayment-the-PM sits relative to SetupMandate-the-flow and
RepeatPayment-the-flow. Read top-to-bottom for the life-cycle.

```
 ┌──────────────────────────────────────────────────────────────┐
 │  1. CIT — Cardholder-Initiated Transaction (Setup)           │
 │  ------------------------------------------------------------│
 │  Flow   : SetupMandate  (connector_flow.rs:23)               │
 │  Or     : Authorize with setup_mandate_details + customer_   │
 │           acceptance set                                     │
 │                                                              │
 │  Request carries real card / wallet data.                    │
 │  Response carries connector_mandate_id                       │
 │    (PaymentsResponseData::TransactionResponse.mandate_       │
 │     reference).                                              │
 │                                                              │
 │  See: pattern_setup_mandate.md                               │
 └──────────────────────────────────────────────────────────────┘
                             │
                             │  mandate_id is stored by Hyperswitch
                             ▼
 ┌──────────────────────────────────────────────────────────────┐
 │  2. MIT — Merchant-Initiated Transaction (Charge)            │
 │  ------------------------------------------------------------│
 │  Two dispatch paths at Authorize time:                       │
 │                                                              │
 │  (a) Preferred path — RepeatPayment flow                     │
 │      Flow    : RepeatPayment (connector_flow.rs:26)          │
 │      Request : RepeatPaymentData<T> (connector_types.rs:2531)│
 │                .mandate_reference: MandateReferenceId        │
 │                  (connector_types.rs:338-342)                │
 │      Cardholder data: none — the connector looks up the      │
 │                       stored instrument by id.               │
 │      See: pattern_repeat_payment_flow.md                     │
 │                                                              │
 │  (b) Legacy / fallback path — Authorize + MandatePayment PM  │
 │      Flow    : Authorize (connector_flow.rs:5)               │
 │      Request : PaymentsAuthorizeData<T>                      │
 │                .payment_method_data = MandatePayment (unit)  │
 │                .mandate_id: Option<MandateIds>               │
 │                  (connector_types.rs:1110)                   │
 │      Connectors that use this path: ACI, Braintree,          │
 │        Zift, Revolv3, Novalnet (legacy dispatch).            │
 │                                                              │
 │      THIS PATTERN documents (b).                             │
 └──────────────────────────────────────────────────────────────┘
                             │
                             │  (mandate revocation is orthogonal)
                             ▼
 ┌──────────────────────────────────────────────────────────────┐
 │  3. Mandate revocation                                       │
 │  ------------------------------------------------------------│
 │  Flow : MandateRevoke  (see pattern_mandate_revoke.md)       │
 │  Independent of PM; runs off the stored mandate id.          │
 └──────────────────────────────────────────────────────────────┘
```

### Why two paths for MIT?

The Authorize+MandatePayment path (2b) is the **older** dispatch style. Newer
connectors (those in `pattern_repeat_payment_flow.md`'s Full Implementation
table) expose a native `RepeatPayment` trait implementation and prefer path
(2a). A connector MAY support **both** paths simultaneously (Cybersource does —
it handles `PaymentMethodData::MandatePayment` inside its repeat-payment
TryFrom at `cybersource/transformers.rs:4295-4304` in addition to the
`connector_mandate_id()` shortcut on line 4292).

When a connector rejects `PaymentMethodData::MandatePayment` in Authorize and
requires RepeatPayment instead, it returns a `not_implemented` error with a
human-readable message. Worldpay is the canonical example:

```rust
// From crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:144
PaymentMethodData::MandatePayment => {
    Err(IntegrationError::not_implemented(
        "MandatePayment should not be used in Authorize flow - use RepeatPayment flow for MIT transactions".to_string()
    ).into())
}
```

---

## Connectors with Full Implementation

The table below lists connectors that **accept** `PaymentMethodData::MandatePayment`
as a real dispatch arm in the Authorize flow (not just as a catch-all
`not_implemented` arm) at the pinned SHA. "Full Implementation" here means
the connector actually builds a different request shape for this variant.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
|-----------|-------------|--------------|-------------|--------------------|-------|
| **ACI** | POST | FormUrlEncoded | `v1/registrations/{mandate_id}/payments` | Reuses `AciPaymentsRequest<T>` with `PaymentDetails::Mandate` and `recurring_type` set | Dispatch arm at `crates/integrations/connector-integration/src/connectors/aci/transformers.rs:729-737`; mandate-specific `TryFrom<(…, MandateIds)>` at `aci/transformers.rs:993-1036` |
| **Braintree** | POST | application/json (GraphQL) | GraphQL endpoint (single URL) | `MandatePaymentRequest = GenericBraintreeRequest<VariablePaymentInput>` aliased at `braintree/transformers.rs:68`; reuses card-request body shape | Authorize-side dispatch at `braintree/transformers.rs:597` and RepeatPayment dispatch at `braintree/transformers.rs:2785-2797`; `MandatePaymentRequest` TryFrom at `braintree/transformers.rs:332-359` |
| **Cybersource** | POST | application/json | `pts/v2/payments/` (same as Authorize) | Reuses `RepeatPaymentInformation::MandatePayment(Box<MandatePaymentInformation>)` at `cybersource/transformers.rs:4262` and `4398`; request struct `MandatePaymentInformation` at `cybersource/transformers.rs:700-704` | Dispatches `MandatePayment` inside RepeatPayment TryFrom at `cybersource/transformers.rs:4295-4304`. Authorize-side treats it as `not_implemented` at `cybersource/transformers.rs:2172` and `2279`, so Cybersource's MandatePayment coverage is *via RepeatPayment*, not via Authorize proper. |
| **Revolv3** | POST | application/json | Authorize endpoint (reuses standard base URL) | `Revolv3PaymentMethodData::MandatePayment` as a unit enum variant at `revolv3/transformers.rs:135`; constructor at `revolv3/transformers.rs:1084-1086` | Dispatches `PaymentMethodData::MandatePayment` inside RepeatPayment TryFrom at `revolv3/transformers.rs:1133` |
| **Zift** | POST | FormUrlEncoded | Zift payment endpoint (see `zift.rs:695`) | Dedicated `ZiftMandatePaymentRequest` struct at `zift/transformers.rs:170`; request enum arm at `zift/transformers.rs:137` | Build at `zift/transformers.rs:736-772`; relies on `connector_mandate_id()` for the token field |
| **Novalnet** | POST | application/json | Novalnet payments endpoint | `NovalNetPaymentData::MandatePayment(NovalnetMandate)` variant at `novalnet/transformers.rs:145` | Build at `novalnet/transformers.rs:2322-2324`; reads mandate id from `RepeatPaymentData.mandate_reference` → `ConnectorMandateId` |
| **Checkout** | POST | application/json | `/payments` | `PaymentSource::MandatePayment(MandateSource)` variant at `checkout/transformers.rs:154`; built at `checkout/transformers.rs:880-884` | Reads from `item.router_data.request.mandate_reference` and sets `merchant_initiated=true` |
| **Multisafepay** | POST | application/json | `orders` | Order type mapped to `Type::Direct` at `multisafepay/transformers.rs:80` when PM is `MandatePayment`; matched at `multisafepay/transformers.rs:325` | Dispatches mandate to a direct-type order |

### Connectors that reject MandatePayment in Authorize

These connectors pattern-match `PaymentMethodData::MandatePayment` but fall
through to a `NotImplemented` error, either because MIT is only supported
through a dedicated RepeatPayment implementation, or because the PM is not
supported at all by the integration:

- Bank of America — `bankofamerica/transformers.rs:600`, `bankofamerica/transformers.rs:1770`
- Fiserv — `fiserv/transformers.rs:541`
- Adyen — `adyen/transformers.rs:3697`, `adyen/transformers.rs:6038` (Adyen handles mandates via a dedicated `AdyenMandatePaymentMethod` at `adyen/transformers.rs:752` inside a different code path)
- Redsys — `redsys/transformers.rs:240`
- Cryptopay — `cryptopay/transformers.rs:102`
- Paypal — `paypal/transformers.rs:1135`, `paypal/transformers.rs:2596`
- Fiuu — `fiuu/transformers.rs:666`
- Trustpay — `trustpay/transformers.rs:1703`
- Razorpay — `razorpay/transformers.rs:298`
- Billwerk — `billwerk/transformers.rs:226`
- Bambora — `bambora/transformers.rs:289`
- Dlocal — `dlocal/transformers.rs:200`
- Nexinets — `nexinets/transformers.rs:732`
- Forte — `forte/transformers.rs:304`
- Wellsfargo — `wellsfargo/transformers.rs:589`
- Mifinity — `mifinity/transformers.rs:240`
- HiPay — `hipay/transformers.rs:587`
- Stax — `stax/transformers.rs:1090`
- Stripe — `stripe/transformers.rs:1514`, `stripe/transformers.rs:4634`, `stripe/transformers.rs:5030` (Stripe handles MIT via its RepeatPayment + `MandateReferenceId` path; see Stripe's RepeatPayment row in `pattern_repeat_payment_flow.md`)
- Noon — `noon/transformers.rs:369`, `noon/transformers.rs:1254`
- Worldpay — `worldpay/transformers.rs:144-148` (explicit "use RepeatPayment" message)
- Volt — `volt/transformers.rs:287`
- Placetopay — `placetopay/transformers.rs:202`
- Loonio — `loonio/transformers.rs:237`

---

## Per-Variant Implementation Notes

`PaymentMethodData::MandatePayment` is a **single unit variant** (the
variant-enumeration table in §Variant Enumeration has one row). The
per-variant notes below therefore cover the operational sub-cases that arise
from how a connector chooses to *interpret* the unit variant.

### Sub-case A — Authorize builds a mandate-specific request

**Used by**: ACI, Braintree, Zift, Revolv3, Novalnet (when the connector
dispatches through either `Authorize` or `RepeatPayment`).

**Expected transformer path**:

1. Match `PaymentMethodData::MandatePayment`.
2. Pull the mandate id from `router_data.request.mandate_id`
   (Option<MandateIds>, `connector_types.rs:1110`) or from
   `router_data.request.connector_mandate_id()` helper
   (`connector_types.rs:1205-1216`) if the router already normalised it.
3. Error with `IntegrationError::MissingRequiredField { field_name:
   "connector_mandate_id", .. }` when the id is absent.
4. Populate the connector-specific mandate request struct.

Illustration (ACI):

```rust
// From crates/integrations/connector-integration/src/connectors/aci/transformers.rs:729
PaymentMethodData::MandatePayment => {
    let mandate_id = item.router_data.request.mandate_id.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "mandate_id",
            context: Default::default(),
        },
    )?;
    Self::try_from((&item, mandate_id))
}
```

Illustration (Zift, which requires the mandate_id via the
`connector_mandate_id()` helper because Zift stores it as a token):

```rust
// From crates/integrations/connector-integration/src/connectors/zift/transformers.rs:736
PaymentMethodData::MandatePayment => {
    // ...
    let mandate_request = ZiftMandatePaymentRequest {
        // ...
        token: Secret::new(item.router_data.request.connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?),
        // ...
    };
    Ok(Self::Mandate(mandate_request))
}
```

### Sub-case B — Authorize rejects the variant, RepeatPayment handles it

**Used by**: Worldpay, Stripe, Adyen, Trustpay (for the MIT path).

**Expected transformer path** inside Authorize: return
`IntegrationError::not_implemented(...)` with a message steering the caller to
RepeatPayment.

Illustration (Worldpay):

```rust
// From crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:144
PaymentMethodData::MandatePayment => {
    Err(IntegrationError::not_implemented(
        "MandatePayment should not be used in Authorize flow - use RepeatPayment flow for MIT transactions".to_string()
    ).into())
}
```

**Pros**: keeps the Authorize transformer free of mandate branching.
**Cons**: requires the caller to switch flow dispatch based on whether
`mandate_id` is populated; this is usually handled at the Grace-UCS router
layer, not in connector code.

### Sub-case C — RepeatPayment explicitly matches MandatePayment

**Used by**: Cybersource, Revolv3. The variant appears as a match arm inside
the connector's RepeatPayment TryFrom — because that flow's request type
(`RepeatPaymentData<T>`) *also* carries a `payment_method_data:
PaymentMethodData<T>` field at
`crates/types-traits/domain_types/src/connector_types.rs:2553`.

Illustration (Cybersource):

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:4294
None => match &item.router_data.request.payment_method_data {
    PaymentMethodData::MandatePayment => {
        let connector_mandate_id =
            item.router_data.request.connector_mandate_id().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                },
            )?;
        Self::try_from((&item, connector_mandate_id))
    }
    // ...
}
```

Note the double-read: the outer `match` first tries
`request.connector_mandate_id()` (which looks at
`mandate_reference.connector_mandate_id`), and the inner PM match only runs
when that outer lookup returned `None` **and** the PM is `MandatePayment`.

---

## Common Implementation Patterns

### Pattern 1 — Unit-variant match and missing-field guard

Every connector that implements MandatePayment in Authorize follows the
same pattern: match the unit variant, fetch the mandate id from the
surrounding request data, error cleanly when it is absent.

```rust
match &router_data.request.payment_method_data {
    PaymentMethodData::MandatePayment => {
        let mandate_ids = router_data
            .request
            .mandate_id
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "mandate_id",
                context: Default::default(),
            })?;
        build_mandate_authorize_request(mandate_ids, router_data)
    }
    PaymentMethodData::Card(_) => { /* ... */ }
    _ => Err(IntegrationError::not_implemented(
        get_unimplemented_payment_method_error_message("my_connector"),
    ).into()),
}
```

- `IntegrationError::MissingRequiredField` — from
  `crates/types-traits/domain_types/src/errors.rs` (the canonical
  request-time error type per `PATTERN_AUTHORING_SPEC.md` §12).
- `get_unimplemented_payment_method_error_message` — utility from
  `domain_types::utils`; see `../utility_functions_reference.md`.

### Pattern 2 — Dispatch via `connector_mandate_id()` helper

Prefer the `connector_mandate_id()` helper on `PaymentsAuthorizeData<T>`
(`connector_types.rs:1205-1216`) over reaching into the nested `MandateIds`
structure directly. The helper normalises the
`MandateReferenceId::ConnectorMandateId` case and returns `None` cleanly for
`NetworkMandateId` and `NetworkTokenWithNTI`.

```rust
// Preferred
let token = item.router_data.request.connector_mandate_id().ok_or(
    IntegrationError::MissingRequiredField {
        field_name: "connector_mandate_id",
        context: Default::default(),
    },
)?;

// Avoid — reaches into internal layout
let token = item
    .router_data
    .request
    .mandate_id
    .as_ref()
    .and_then(|m| m.mandate_reference_id.as_ref())
    .and_then(|r| match r {
        MandateReferenceId::ConnectorMandateId(c) => c.get_connector_mandate_id(),
        _ => None,
    })
    .ok_or(/* ... */)?;
```

### Pattern 3 — Dual-path handling (Authorize AND RepeatPayment)

A connector that implements both paths (Cybersource) should:

1. In Authorize: treat `MandatePayment` as `not_implemented` (it belongs in
   RepeatPayment).
2. In RepeatPayment: match `MandatePayment` as a valid dispatch arm.

This cleanly separates the two flows and keeps each transformer's
responsibility narrow.

### Pattern 4 — Network-mandate interplay

When the connector also supports network-transaction-id-based MIT
(`MandateReferenceId::NetworkMandateId(String)` at
`connector_types.rs:340`), it should *not* match `MandatePayment` for that
case — NTID-based MIT uses `PaymentMethodData::CardDetailsForNetworkTransactionId`
instead, because the cardholder account number is still needed on the wire.

See Revolv3 for a clean example of the split:

- `PaymentMethodData::CardDetailsForNetworkTransactionId` →
  `Revolv3PaymentMethodData::set_credit_card_data_for_ntid(...)`
  (`revolv3/transformers.rs:1120-1132`).
- `PaymentMethodData::MandatePayment` →
  `Revolv3PaymentMethodData::set_mandate_data()`
  (`revolv3/transformers.rs:1133`).

---

## Code Examples

### Example 1 — ACI: full Authorize-side dispatch

```rust
// From crates/integrations/connector-integration/src/connectors/aci/transformers.rs:729
PaymentMethodData::MandatePayment => {
    let mandate_id = item.router_data.request.mandate_id.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "mandate_id",
            context: Default::default(),
        },
    )?;
    Self::try_from((&item, mandate_id))
}
```

And the mandate-specific builder:

```rust
// From crates/integrations/connector-integration/src/connectors/aci/transformers.rs:993
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &AciRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>,
        MandateIds,
    )> for AciPaymentsRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            &AciRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>,
            MandateIds,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, _mandate_data) = value;
        let instruction = get_instruction_details(item);
        let txn_details = get_transaction_details(item)?;
        let recurring_type = get_recurring_type(item);

        Ok(Self {
            txn_details,
            payment_method: PaymentDetails::Mandate,
            instruction,
            shopper_result_url: item.router_data.request.router_return_url.clone(),
            three_ds_two_enrolled: None,
            recurring_type,
        })
    }
}
```

### Example 2 — Braintree: MIT via RepeatPayment

```rust
// From crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:2785
PaymentMethodData::MandatePayment => {
    let connector_mandate_id = item.router_data.request.connector_mandate_id().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "connector_mandate_id",
            context: Default::default(),
        },
    )?;
    Ok(Self::Mandate(MandatePaymentRequest::try_from((
        item,
        connector_mandate_id,
        metadata,
    ))?))
}
```

The `MandatePaymentRequest` is a type alias for a `GenericBraintreeRequest`
that reuses the standard charge-credit-card mutation; the only difference is
that the request carries a stored payment-method token instead of fresh card
data:

```rust
// From crates/integrations/connector-integration/src/connectors/braintree/transformers.rs:68
pub type MandatePaymentRequest = GenericBraintreeRequest<VariablePaymentInput>;
```

### Example 3 — Worldpay: explicit rejection

```rust
// From crates/integrations/connector-integration/src/connectors/worldpay/transformers.rs:144
PaymentMethodData::MandatePayment => {
    Err(IntegrationError::not_implemented(
        "MandatePayment should not be used in Authorize flow - use RepeatPayment flow for MIT transactions".to_string()
    ).into())
}
```

### Example 4 — Zift: FormUrlEncoded MIT

```rust
// From crates/integrations/connector-integration/src/connectors/zift/transformers.rs:736
PaymentMethodData::MandatePayment => {
    let card_details = match &item.router_data.request.payment_method_data {
        PaymentMethodData::Card(card) => Ok(card),
        _ => Err(error_stack::report!(IntegrationError::NotSupported {
            message: "Payment Method Not Supported".to_string(),
            connector: "Zift",
            context: Default::default()
        })),
    }?;

    let mandate_request = ZiftMandatePaymentRequest {
        request_type,
        auth,
        account_type: AccountType::PaymentCard,
        token: Secret::new(item.router_data.request.connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?),
        account_accessory: card_details
            .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
        // ...
        transaction_category_type: TransactionCategoryType::Recurring,
        sequence_number: 2, // Its required for MIT
        // ...
    };
    Ok(Self::Mandate(mandate_request))
}
```

Note Zift's quirk: it reads `PaymentMethodData::Card` *inside* a branch that
already matched `PaymentMethodData::MandatePayment`. This works when the
router populates the Card data alongside the MandatePayment signal, but is
brittle — it is listed as a connector-specific quirk rather than a
reusable pattern.

### Example 5 — Revolv3: unit-to-unit mapping

```rust
// From crates/integrations/connector-integration/src/connectors/revolv3/transformers.rs:1133
PaymentMethodData::MandatePayment => Revolv3PaymentMethodData::set_mandate_data()?,
```

Where `set_mandate_data` is:

```rust
// From crates/integrations/connector-integration/src/connectors/revolv3/transformers.rs:1084
pub fn set_mandate_data() -> Result<Self, error_stack::Report<IntegrationError>> {
    Ok(Self::MandatePayment)
}
```

And the target variant is itself a unit variant of the connector's own
`Revolv3PaymentMethodData` enum at
`crates/integrations/connector-integration/src/connectors/revolv3/transformers.rs:135`.

### Example 6 — Checkout: source-style mandate

```rust
// From crates/integrations/connector-integration/src/connectors/checkout/transformers.rs:880
let mandate_source = PaymentSource::MandatePayment(MandateSource {
    source_type: CheckoutSourceTypes::SourceId,
    source_id: mandate_data.get_connector_mandate_id(),
    billing_address: billing_details,
});
```

Note Checkout reads from `request.mandate_reference` (because this branch is
inside RepeatPayment), not from `request.mandate_id` — per the
`RepeatPaymentData` shape at `connector_types.rs:2532`.

---

## Best Practices

- **Prefer the RepeatPayment flow for new connector integrations.** It has a
  richer, MIT-aware request type (`RepeatPaymentData<T>` at
  `connector_types.rs:2531`) and keeps the Authorize transformer simpler. See
  `pattern_repeat_payment_flow.md` for the flow-level pattern.
- **Use `connector_mandate_id()` instead of hand-walking `MandateIds`.**
  The helper at `connector_types.rs:1205-1216` (Authorize) and at
  `connector_types.rs:2617-2625` (RepeatPayment) normalises
  `MandateReferenceId::ConnectorMandateId` for you.
- **Error with `IntegrationError::MissingRequiredField { field_name:
  "connector_mandate_id", .. }`** when the mandate id is absent. This is the
  canonical error per `PATTERN_AUTHORING_SPEC.md` §12. ACI
  (`aci/transformers.rs:731-735`), Braintree
  (`braintree/transformers.rs:2786-2790`), Zift
  (`zift/transformers.rs:750-755`), and Cybersource
  (`cybersource/transformers.rs:4297-4301`) all do this.
- **Never populate `mandate_reference` on the success response for a MIT
  charge.** The mandate already exists; overwriting it causes the router to
  rotate the stored id. See Cybersource's RepeatPayment response builder at
  `cybersource/transformers.rs:4398` for the correct `None` default.
- **Distinguish `MandatePayment` from `CardDetailsForNetworkTransactionId`.**
  NTID-based MIT carries the card PAN on the wire and therefore needs the
  Card variant with NTID metadata — not `MandatePayment`. Revolv3's split at
  `revolv3/transformers.rs:1120-1133` illustrates both branches side-by-side.
- **If your connector only supports MIT via RepeatPayment, reject
  `MandatePayment` in Authorize with an actionable message**, as Worldpay
  does (`worldpay/transformers.rs:144-148`).
- **Do not hardcode `AttemptStatus` in response TryFrom blocks.** Follow the
  status-mapping guidance in `authorize/card/pattern_authorize_card.md`
  §Response Patterns (banned anti-pattern #1 in
  `PATTERN_AUTHORING_SPEC.md` §11).

---

## Common Errors

### Error 1 — "connector_mandate_id missing" from an un-checked Option

**Problem.** A connector builds the mandate request assuming
`request.mandate_id` is `Some`, panics in dev, or emits an opaque
"InternalServerError" in production.

**Solution.** Always guard the `Option<MandateIds>` with
`IntegrationError::MissingRequiredField`. Pattern 1 above shows the
canonical shape. Cite: ACI does this at
`aci/transformers.rs:731-735`, Braintree at
`braintree/transformers.rs:2786-2790`.

### Error 2 — Returning `NotImplemented` without telling the caller to use RepeatPayment

**Problem.** A connector rejects `MandatePayment` in Authorize with a generic
"Payment method not implemented" message. The caller has no way to know that
the same connector accepts MIT through its RepeatPayment implementation.

**Solution.** Use an actionable message pointing to `RepeatPayment`, matching
Worldpay's wording at `worldpay/transformers.rs:144-148`:

```rust
Err(IntegrationError::not_implemented(
    "MandatePayment should not be used in Authorize flow - use RepeatPayment flow for MIT transactions".to_string()
).into())
```

### Error 3 — Conflating `MandatePayment` with `NetworkMandateId`

**Problem.** A connector matches `PaymentMethodData::MandatePayment` and
then reads `request.get_optional_network_transaction_id()`. The NTID is
`None` for a true MandatePayment charge (because NTID belongs to the
`CardDetailsForNetworkTransactionId` path), and the connector errors
confusingly.

**Solution.** Route on both the PM variant *and* the `MandateReferenceId`
discriminant. For `MandatePayment`, require
`MandateReferenceId::ConnectorMandateId`; for NTID-MIT, require the Card
variant and `MandateReferenceId::NetworkMandateId`. Revolv3 splits these at
`revolv3/transformers.rs:1120-1133`.

### Error 4 — Dispatching MandatePayment inside RepeatPayment without the Option check

**Problem.** A connector's RepeatPayment TryFrom assumes the outer
`request.connector_mandate_id()` is `Some`, and only checks the PM variant
inside the inner match. When the helper returns `None` and the PM is *not*
`MandatePayment`, the fallthrough error is misleading.

**Solution.** Cybersource's two-stage match at
`cybersource/transformers.rs:4292-4328` is the canonical template: first
try the helper; if `None`, dispatch on the PM variant; each PM arm that is
unsupported errors with a specific `not_implemented` message.

### Error 5 — Populating `mandate_reference` on the success response

**Problem.** A connector's success TryFrom for
`PaymentsResponseData::TransactionResponse` copies the incoming
`connector_mandate_id` back into the response's `mandate_reference` field,
causing the router to treat it as a new mandate and rotate the stored id.

**Solution.** Leave `mandate_reference: None` on MIT responses. Mandate
rotation is handled explicitly by a separate SetupMandate flow; see
`pattern_setup_mandate.md`.

---

## Cross-References

### Pattern index

- Patterns root: [../../README.md](../../README.md)
- Authorize PM index: [../README.md](../README.md)
- Authoring spec: [../../PATTERN_AUTHORING_SPEC.md](../../PATTERN_AUTHORING_SPEC.md)

### Related flow patterns (read-only — do not edit)

- [../../pattern_setup_mandate.md](../../pattern_setup_mandate.md) — the CIT
  side of the lifecycle that produces the mandate id consumed here.
- [../../pattern_repeat_payment_flow.md](../../pattern_repeat_payment_flow.md) —
  the preferred MIT dispatch path. Connectors listed as "Full Implementation"
  there generally treat `MandatePayment` inside Authorize as
  `not_implemented`.
- [../../pattern_mandate_revoke.md](../../pattern_mandate_revoke.md) — tear-down
  flow for a stored mandate; orthogonal to both CIT setup and MIT charge.

### Sibling PM patterns

- [../card/pattern_authorize_card.md](../card/pattern_authorize_card.md) —
  the gold PM pattern; `CardDetailsForNetworkTransactionId` (the NTID
  path) is discussed there as a Card-family variant, not in this pattern.
- [../wallet/pattern_authorize_wallet.md](../wallet/pattern_authorize_wallet.md)
  — the wallet path occasionally intersects with mandate storage (tokenised
  wallet payments).
- [../bank_debit/pattern_authorize_bank_debit.md](../bank_debit/pattern_authorize_bank_debit.md)
  — mandate-based recurring debits use the BankDebit PM, not MandatePayment.

### Canonical types

- Canonical `RouterDataV2` signatures: see
  [../../PATTERN_AUTHORING_SPEC.md](../../PATTERN_AUTHORING_SPEC.md) §7.
- Canonical mandate types: `MandateIds`, `MandateReferenceId`,
  `ConnectorMandateReferenceId` at
  `crates/types-traits/domain_types/src/connector_types.rs:257-348`.
- Canonical mandate domain types (customer acceptance, mandate data shape):
  `crates/types-traits/domain_types/src/mandates.rs:10-87`.

### Utility helpers

- Utility reference: [../../utility_functions_reference.md](../../utility_functions_reference.md)
  — covers `get_unimplemented_payment_method_error_message`, referenced by
  every connector that emits an unsupported-PM error.

### Types reference

- Connector types overview: [../../../types/types.md](../../../types/types.md).
