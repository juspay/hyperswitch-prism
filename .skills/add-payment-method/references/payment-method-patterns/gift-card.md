# Gift Card Authorize Pattern Reference

## Payment Method: Gift Card (Givex, PaySafeCard)

Prepaid stored-value card payments. Typically synchronous.
Givex requires card number + CVC. PaySafeCard is redirect/token-based.

## GiftCardData Variants

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs
pub enum GiftCardData {
    Givex(GiftCardDetails),
    PaySafeCard {},
}

pub struct GiftCardDetails {
    pub number: Secret<String>,
    pub cvc: Secret<String>,
}
```

## Connector Support

| Connector | Givex | PaySafeCard | Notes |
|-----------|-------|-------------|-------|
| Adyen | Yes | Yes | Primary reference implementation |
| Others | NotImplemented | NotImplemented | Return error |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::GiftCard(ref gift_card_data) => {
    ConnectorPaymentMethod::try_from(gift_card_data.as_ref())?
}
```

## Adyen TryFrom Pattern

```rust
impl TryFrom<&GiftCardData> for AdyenPaymentMethod<T> {
    fn try_from(gift_card_data: &GiftCardData) -> Result<Self, Self::Error> {
        match gift_card_data {
            GiftCardData::PaySafeCard {} => Ok(Self::PaySafeCard),
            GiftCardData::Givex(givex_data) => {
                let gift_card_pm = AdyenGiftCardData {
                    brand: GiftCardBrand::Givex,
                    number: givex_data.number.clone(),
                    cvc: givex_data.cvc.clone(),
                };
                Ok(Self::AdyenGiftCard(Box::new(gift_card_pm)))
            }
        }
    }
}
```

## Adyen Connector Structs

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenGiftCardData {
    brand: GiftCardBrand,
    number: Secret<String>,
    cvc: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GiftCardBrand {
    Givex,
    Auriga,
    Babygiftcard,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AdyenPaymentMethod<T> {
    #[serde(rename = "giftcard")]
    AdyenGiftCard(Box<AdyenGiftCardData>),
    #[serde(rename = "paysafecard")]
    PaySafeCard,
}
```

## Full Request Construction

```rust
// Extract from PaymentMethodData
let payment_method = match &router_data.request.payment_method_data {
    PaymentMethodData::GiftCard(gift_card_data) => {
        ConnectorPaymentMethod::try_from(gift_card_data.as_ref())?
    }
    _ => return Err(ConnectorError::NotImplemented(...)),
};

// Build request with standard fields
Ok(ConnectorAuthorizeRequest {
    amount: item.amount,
    currency: router_data.request.currency.to_string(),
    payment_method,
    reference: router_data.resource_common_data.connector_request_reference_id.clone(),
    return_url: router_data.request.get_router_return_url()?,
    shopper_email: router_data.resource_common_data.get_optional_billing_email(),
    telephone_number: router_data.resource_common_data.get_optional_billing_phone_number(),
    ...
})
```

## NotImplemented Pattern

```rust
impl TryFrom<&GiftCardData> for OtherConnectorRequest<T> {
    fn try_from(value: &GiftCardData) -> Result<Self, Self::Error> {
        match value {
            GiftCardData::Givex(_) | GiftCardData::PaySafeCard {} => {
                Err(ConnectorError::NotImplemented(
                    get_unimplemented_payment_method_error_message("ConnectorName"),
                ).into())
            }
        }
    }
}
```

## Status Mapping

Gift cards are typically synchronous:
```rust
match status {
    Succeeded => AttemptStatus::Charged,
    Pending => AttemptStatus::Pending,
    Failed => AttemptStatus::Failure,
}
```

## Key Implementation Notes

- Givex requires `number` and `cvc` (both `Secret<String>`)
- PaySafeCard has no additional data fields (unit struct)
- Always use `Secret<String>` for card number/CVC; never log raw values
- Gift card payments are typically synchronous (no redirect needed)
- Handle partial redemption if connector supports it
- For macro usage, see `macro-reference.md`
