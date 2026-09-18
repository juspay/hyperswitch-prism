# Card-With-No-CVC Authorize Pattern Reference

## Payment Method: `PaymentMethodData::CardWithNoCvc`

Raw card details **without** a CVC. This is the one `PaymentMethodData` variant with no
pattern file in the wider rulesbook set (`grace/rulesbook/codegen/guides/patterns/authorize/`),
so this file is the canonical reference for it. For everything that is not CVC-specific,
follow `card.md` -- the request-building, amount-unit and status-mapping rules are identical.

Do **not** confuse it with:
- `PaymentMethodData::Card(Card<T>)` -- the normal card variant, carries `card_cvc`.
- `PaymentMethodData::CardDetailsForNetworkTransactionId(..)` -- MIT re-use keyed on a stored
  network transaction ID (see `card-ntid.md`).
- `ProxyCardDetails` / `PaymentMethodDataAction::CardProxy` -- the proxy-card path, a different
  server-side action.

## The Rust Type

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs, `pub struct CardWithNoCvc`
pub struct CardWithNoCvc {
    pub card_number: cards::CardNumber,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_issuer: Option<String>,
    pub card_network: Option<CardNetwork>,
    pub card_type: Option<String>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<String>,
    pub nick_name: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub co_badged_card_data: Option<CoBadgedCardData>,
}
```

Eleven fields. Only `card_number`, `card_exp_month` and `card_exp_year` are non-optional --
there is **no** `card_cvc` field, so any helper or connector struct that requires a CVC cannot
be reached from this variant. `CardWithNoCvc` has its own `get_card_issuer()` inherent method,
the same as `Card` and `NetworkTokenData`.

## Where It Arrives From

The gRPC oneof field is `card_with_no_cvc` (field 5 of `message PaymentMethod`,
`crates/types-traits/grpc-api-types/proto/payment_methods.proto`), carrying
`message CardDetailsWithNoCvc`. Note the proto message has ten fields and no
`co_badged_card_data`.

```json
{"payment_method": {"card_with_no_cvc": {
  "card_number": {"value": "4111111111111111"},
  "card_exp_month": {"value": "03"},
  "card_exp_year": {"value": "2030"},
  "card_holder_name": {"value": "Jane Doe"}
}}}
```

The server maps it through `PaymentMethodDataAction::CardWithNoCvc`
(`crates/types-traits/domain_types/src/types.rs`, `pub enum PaymentMethodDataAction`) on more
than one path in `crates/grpc-server/grpc-server/src/server/payments.rs` -- payment authorize,
setup-recurring, and the payment-method-token flow -- so a connector that accepts it may see it
outside Authorize. Treat "which flows accept it" as a per-connector decision driven by the spec.

## Connector Support At HEAD

36 connector transformer files name `PaymentMethodData::CardWithNoCvc`. Of those, exactly two
handle it positively:

| Connector | File | Shape |
|---|---|---|
| Fiservcommercehub | `connectors/fiservcommercehub/transformers.rs` | `PaymentMethodData::CardWithNoCvc(card) => { let encrypted_card = encrypt_card_data_no_cvc(card, key_id, &public_key_der)?; ... }` -- a dedicated no-CVC encryption helper parallel to the normal card one |
| Juspay | `connectors/juspay/transformers.rs`, `refreshable_card` | Accepts **only** this variant on the `RefreshPaymentMethod` (account-updater) flow and rejects everything else |

Every other file lists it inside a grouped rejection arm. That grouped arm is the default you
should copy unless the vendor spec explicitly supports CVC-less card entry.

## Rejection Arm (the common case)

`CardWithNoCvc` is normally folded into the same grouped `NotImplemented` arm as the other
unsupported variants -- one arm, many `|`-separated patterns:

```rust
PaymentMethodData::CardRedirect(_)
| PaymentMethodData::PayLater(_)
| PaymentMethodData::BankRedirect(_)
| PaymentMethodData::Crypto(_)
| PaymentMethodData::MandatePayment
| PaymentMethodData::Reward
| PaymentMethodData::RealTimePayment(_)
| PaymentMethodData::CardWithNoCvc(_)
| PaymentMethodData::MobilePayment(_)
| PaymentMethodData::Upi(_)
| PaymentMethodData::GiftCard(_)
| PaymentMethodData::OpenBanking(_)
| PaymentMethodData::PaymentMethodToken(_)
| PaymentMethodData::NetworkToken(_)
| PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
| PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
    Err(IntegrationError::NotImplemented(
        crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
        Default::default(),
    ))?
}
```

(Verbatim shape from `connectors/dlocal/transformers.rs`.) Listing the variants explicitly
rather than using `_ =>` is what makes the next `PaymentMethodData` variant a compile error
instead of a silent drop.

When the restriction is "this connector/flow accepts CVC-less cards only", the inverse
rejection uses the struct variant `IntegrationError::NotSupported`, not `NotImplemented`:

```rust
// connectors/juspay/transformers.rs, `refreshable_card`
_ => Err(error_stack::report!(errors::IntegrationError::NotSupported {
    message: "account updater accepts card_with_no_cvc only".to_string(),
    connector: "juspay",
    context: Default::default(),
})),
```

## Support Arm

```rust
PaymentMethodData::CardWithNoCvc(card) => {
    // No CVC exists. Never synthesise one ("000", "") to satisfy a connector struct --
    // build a request type whose CVC field is absent, or return NotImplemented.
    Ok(ConnectorPaymentsRequest {
        card_number: card.card_number.clone(),
        expiry_month: card.card_exp_month.clone(),
        expiry_year: card.card_exp_year.clone(),
        card_holder_name: card.card_holder_name.clone(),
        ..
    })
}
```

## Key Implementation Notes

- **Never fabricate a CVC.** If the connector's card object requires one, this variant is
  genuinely unsupported for that connector -- reject it.
- `card_number` is `cards::CardNumber` and the expiry fields are `Secret<String>`; the same
  masking rules as `card.md` apply. Do not `Debug`-format the whole `PaymentMethodData`
  in an error path -- some variants carry unmasked fields.
- Only add a positive arm when the vendor spec documents CVC-less card acceptance (stored
  credential / MIT / account-updater style). Otherwise the grouped rejection is correct and
  is what 34 of 36 connectors do.
- The field-probe's canonical sample payload for this variant lives at
  `crates/internal/field-probe/src/sample_data.rs` (`PmVariant::CardWithNoCvc`).
- For macro usage, see `macro-reference.md`. For the rest of the card request shape, see
  `card.md`.
