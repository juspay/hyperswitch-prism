# BNPL (Buy Now Pay Later) Authorize Pattern Reference

## Payment Method: BNPL / PayLater

Installment-based payments requiring customer redirect to BNPL provider for approval.
Requires extensive customer data (email, phone, billing/shipping addresses).

## PayLaterData Variants

| Variant | Description | Common Connectors |
|---------|-------------|-------------------|
| `KlarnaRedirect` | Klarna via redirect | Adyen, Stripe |
| `KlarnaSdk` | Klarna via SDK | Adyen |
| `AffirmRedirect` | Affirm (US) | Adyen, Stripe |
| `AfterpayClearpayRedirect` | Afterpay/Clearpay | Adyen, Stripe |
| `PayBrightRedirect` | PayBright (Canada) | Adyen |
| `WalleyRedirect` | Walley (Nordics) | Adyen |
| `AlmaRedirect` | Alma (France) | Adyen |
| `AtomeRedirect` | Atome (SE Asia) | Adyen |

## PaymentMethodData Match Arm

```rust
PaymentMethodData::PayLater(ref pay_later_data) => {
    // Build connector-specific BNPL request
    ConnectorPaymentMethod::try_from((router_data, pay_later_data))?
}
```

## Adyen TryFrom Pattern (with Required Field Validation)

```rust
impl TryFrom<(&RouterDataV2<...>, &PayLaterData)> for AdyenPaymentMethod<T> {
    fn try_from((router_data, pay_later_data): (...)) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_country()?;
                Ok(Self::Klarna)
            }
            PayLaterData::AffirmRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_address()?;
                Ok(Self::Affirm)
            }
            PayLaterData::AfterpayClearpayRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_address()?;
                router_data.resource_common_data.get_shipping_address()?;
                let country = router_data.resource_common_data.get_billing_country()?;
                match country {
                    CountryAlpha2::GB | CountryAlpha2::ES | CountryAlpha2::FR | CountryAlpha2::IT => {
                        Ok(Self::ClearPay)  // UK/EU = Clearpay
                    }
                    _ => Ok(Self::AfterPay),  // AU/NZ/US = Afterpay
                }
            }
            PayLaterData::AlmaRedirect { .. } => {
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_address()?;
                router_data.resource_common_data.get_shipping_address()?;
                Ok(Self::Alma)
            }
            PayLaterData::AtomeRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_address()?;
                Ok(Self::Atome)
            }
            _ => Err(IntegrationError::NotImplemented(..., Default::default())),
        }
    }
}
```

## Required Fields per BNPL Type

| Type | email | full_name | phone | billing_addr | shipping_addr | country |
|------|-------|-----------|-------|-------------|--------------|---------|
| Klarna | Yes | - | - | - | - | Yes |
| Affirm | Yes | Yes | Yes | Yes | - | - |
| Afterpay/Clearpay | Yes | Yes | - | Yes | Yes | Yes |
| PayBright | Yes | Yes | Yes | Yes | Yes | Yes |
| Walley | Yes | - | Yes | - | - | - |
| Alma | Yes | - | Yes | Yes | Yes | - |
| Atome | Yes | Yes | Yes | Yes | - | - |

## Stripe Pattern

```rust
impl TryFrom<&PayLaterData> for StripePaymentMethodType {
    fn try_from(pay_later_data: &PayLaterData) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => Ok(Self::Klarna),
            PayLaterData::AffirmRedirect {} => Ok(Self::Affirm),
            PayLaterData::AfterpayClearpayRedirect { .. } => Ok(Self::AfterpayClearpay),
            _ => Err(IntegrationError::NotImplemented(..., Default::default())),
        }
    }
}
```

Stripe requires shipping address validation for AfterpayClearpay:
```rust
// Validate shipping.address.line1, shipping.address.country, shipping.address.zip
validate_shipping_address_against_payment_method(&shipping_address, Some(&StripePaymentMethodType::AfterpayClearpay))?;
```

## Adyen Connector Payment Method Enum

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AdyenPaymentMethod<T> {
    #[serde(rename = "klarna")]     Klarna,
    #[serde(rename = "affirm")]     AdyenAffirm,
    #[serde(rename = "afterpaytouch")] AfterPay,
    #[serde(rename = "clearpay")]   ClearPay,
    #[serde(rename = "paybright")]  PayBright,
    #[serde(rename = "walley")]     Walley,
    #[serde(rename = "alma")]       AlmaPayLater,
    #[serde(rename = "atome")]      Atome,
}
```

## Redirect Response Handling

```rust
let status = match response.status {
    ConnectorStatus::Pending => AttemptStatus::AuthenticationPending,
    ConnectorStatus::Authorized => AttemptStatus::Authorized,
    ConnectorStatus::Captured => AttemptStatus::Charged,
    ConnectorStatus::Failed => AttemptStatus::Failure,
};

let redirection_data = response.redirect_url.as_ref().map(|url| {
    RedirectForm::Uri { uri: url.clone() }
});
```

## Adyen Status Mapping

```rust
match adyen_status {
    RedirectShopper | ChallengeShopper | PresentToShopper => AttemptStatus::AuthenticationPending,
    Authorised => if is_manual_capture { AttemptStatus::Authorized } else { AttemptStatus::Charged },
    Cancelled => AttemptStatus::Voided,
    Error | Refused => AttemptStatus::Failure,
    Pending => AttemptStatus::Pending,
}
```

## Key Implementation Notes

- BNPL is always redirect-based and async; implement PSync and webhooks
- Afterpay vs Clearpay is region-dependent (use billing country)
- KlarnaSdk requires a non-empty `token` field
- Always validate all required fields before building the request
- For macro usage, see `macro-reference.md`
