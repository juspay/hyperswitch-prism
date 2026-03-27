# Bank Transfer Authorize Flow Pattern

## Table of Contents

- [Overview](#overview)
- [Quick Reference](#quick-reference)
- [Supported Connectors](#supported-connectors)
- [Payment Method Variants](#payment-method-variants)
- [Request Patterns](#request-patterns)
  - [Standard JSON Pattern](#standard-json-pattern)
  - [Form-Encoded Pattern](#form-encoded-pattern)
  - [Multi-Method Request Pattern](#multi-method-request-pattern)
- [Response Patterns](#response-patterns)
  - [Synchronous Response](#synchronous-response)
  - [Async Response with Bank Instructions](#async-response-with-bank-instructions)
  - [Redirect Response](#redirect-response)
- [Implementation Templates](#implementation-templates)
  - [Macro-Based Implementation](#macro-based-implementation)
  - [Manual Implementation](#manual-implementation)
- [Sub-type Variations](#sub-type-variations)
- [Webhook Handling](#webhook-handling)
- [Psync Behavior](#psync-behavior)
- [Common Pitfalls](#common-pitfalls)
- [Testing Patterns](#testing-patterns)
- [Implementation Checklist](#implementation-checklist)

---

## Overview

Bank Transfer is a payment method where customers transfer funds directly from their bank account to the merchant. Unlike Bank Redirect (which redirects customers to their bank's website), Bank Transfer typically involves:

1. **Customer-initiated transfers**: Customer manually transfers funds using provided bank details
2. **Reference-based matching**: Payments are matched using reference numbers or entity IDs
3. **Async confirmation**: Transfers may take hours or days to complete
4. **Region-specific variants**: Different bank transfer systems per country/region

**Key Characteristics:**
- No sensitive bank credentials collected by merchant
- Customer performs transfer outside of the payment flow
- Requires clear payment instructions to customer
- Heavy reliance on webhooks for status updates

---

## Quick Reference

| Aspect | Value |
|--------|-------|
| **Payment Method Type** | `BankTransfer` |
| **Response Type** | Async (typically requires webhook) |
| **Amount Unit** | Minor (most connectors) |
| **3DS Support** | No |
| **Mandate Support** | No |
| **Refund Support** | Connector-dependent |

### Bank Transfer Variants Support Matrix

| Variant | Stripe | Adyen | Trustpay | PayPal | Description |
|---------|--------|-------|----------|--------|-------------|
| `AchBankTransfer` | ✅ | ❌ | ❌ | ❌ | US ACH credit transfer |
| `SepaBankTransfer` | ✅ | ❌ | ❌ | ❌ | EU SEPA transfer |
| `BacsBankTransfer` | ✅ | ❌ | ❌ | ❌ | UK BACS transfer |
| `MultibancoBankTransfer` | ✅ | ❌ | ❌ | ❌ | Portugal Multibanco |
| `PermataBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia Permata |
| `BcaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia BCA |
| `BniVaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia BNI VA |
| `BriVaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia BRI VA |
| `CimbVaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia CIMB VA |
| `DanamonVaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia Danamon |
| `MandiriVaBankTransfer` | ❌ | ✅ | ❌ | ❌ | Indonesia Mandiri VA |
| `Pix` | ❌ | ✅ | ❌ | ✅ | Brazil Pix |
| `Pse` | ❌ | ❌ | ❌ | ❌ | Colombia PSE |
| `LocalBankTransfer` | ❌ | ❌ | ❌ | ❌ | Generic local transfer |
| `InstantBankTransfer` | ❌ | ❌ | ❌ | ❌ | Generic instant transfer |

---

## Supported Connectors

| Connector | Request Format | Response Type | Webhook Required | Notes |
|-----------|---------------|---------------|------------------|-------|
| **Stripe** | FormUrlEncoded | Async | Yes | Uses `customer_balance` payment method |
| **Adyen** | JSON | Async/Redirect | Yes | Supports Indonesian VA transfers |
| **Trustpay** | JSON/Form | Async | Yes | European bank transfers |
| **PayPal** | JSON | Async | Yes | Limited bank transfer support |
| **Razorpay** | JSON | N/A | N/A | Not implemented |
| **Worldpay** | N/A | N/A | N/A | Not supported |
| **Fiserv** | N/A | N/A | N/A | Not supported |
| **Cybersource** | N/A | N/A | N/A | Not supported |

---

## Payment Method Variants

### Rust Enum Definition

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BankTransferData {
    AchBankTransfer,
    SepaBankTransfer,
    BacsBankTransfer,
    MultibancoBankTransfer,
    PermataBankTransfer,
    BcaBankTransfer,
    BniVaBankTransfer,
    BriVaBankTransfer,
    CimbVaBankTransfer,
    DanamonVaBankTransfer,
    MandiriVaBankTransfer,
    Pix { pix_key: Option<String>, pix_key_type: Option<String> },
    Pse,
    LocalBankTransfer { bank_name: Option<String> },
    InstantBankTransfer,
    InstantBankTransferFinland,
    InstantBankTransferPoland,
}
```

---

## Request Patterns

### Standard JSON Pattern

**Applies to**: Adyen, Trustpay

**Characteristics**:
- Request Format: JSON
- Response Type: Async with redirect or instructions
- Amount Unit: Minor

#### Adyen Bank Transfer Request (Indonesian VA)

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1580-1618

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankTransferData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_transfer_data, item): (
            &BankTransferData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_transfer_data {
            BankTransferData::PermataBankTransfer {} => Ok(Self::PermataBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BcaBankTransfer {} => Ok(Self::BcaBankTransfer(Box::new(
                DokuBankData::try_from(item)?,
            ))),
            BankTransferData::BniVaBankTransfer {} => {
                Ok(Self::BniVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::BriVaBankTransfer {} => {
                Ok(Self::BriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::CimbVaBankTransfer {} => {
                Ok(Self::CimbVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::DanamonVaBankTransfer {} => {
                Ok(Self::DanamonVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::MandiriVaBankTransfer {} => {
                Ok(Self::MandiriVa(Box::new(DokuBankData::try_from(item)?)))
            }
            BankTransferData::Pix { .. } => Ok(Self::Pix),
            BankTransferData::AchBankTransfer {}
            | BankTransferData::SepaBankTransfer {}
            | BankTransferData::BacsBankTransfer {}
            | BankTransferData::MultibancoBankTransfer {}
            | BankTransferData::Pse {}
            | BankTransferData::LocalBankTransfer { .. }
            | BankTransferData::InstantBankTransfer {}
            | BankTransferData::InstantBankTransferFinland {}
            | BankTransferData::InstantBankTransferPoland {} => {
                Err(errors::ConnectorError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Adyen"),
                )
                .into())
            }
        }
    }
}

// DokuBankData structure for Indonesian bank transfers
#[derive(Debug, Serialize)]
pub struct DokuBankData {
    #[serde(rename = "type")]
    pub payment_method_type: String,
    pub shopper_email: Email,
}
```

**JSON Request Structure:**
```json
{
  "amount": {
    "currency": "IDR",
    "value": 100000
  },
  "reference": "order-123",
  "paymentMethod": {
    "type": "permata"
  },
  "shopperEmail": "customer@example.com",
  "returnUrl": "https://example.com/return"
}
```

---

### Form-Encoded Pattern

**Applies to**: Stripe

**Characteristics**:
- Request Format: `application/x-www-form-urlencoded`
- Response Type: Async with bank instructions
- Amount Unit: Minor
- Uses `customer_balance` as base payment method type

#### Stripe Bank Transfer Request

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1354-1430

PaymentMethodData::BankTransfer(bank_transfer_data) => match bank_transfer_data.deref() {
    payment_method_data::BankTransferData::AchBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(StripeBankTransferData::AchBankTransfer(
            Box::new(AchTransferData {
                payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                bank_transfer_type: StripeCreditTransferTypes::AchCreditTransfer,
                payment_method_type: StripePaymentMethodType::CustomerBalance,
                balance_funding_type: BankTransferType::BankTransfers,
            }),
        )),
        None,
        StripeBillingAddress::default(),
    )),
    payment_method_data::BankTransferData::MultibancoBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(
            StripeBankTransferData::MultibancoBankTransfers(Box::new(
                MultibancoTransferData {
                    payment_method_data_type: StripeCreditTransferTypes::Multibanco,
                    payment_method_type: StripeCreditTransferTypes::Multibanco,
                    email: payment_request_details.billing_address.email.ok_or(
                        ConnectorError::MissingRequiredField {
                            field_name: "billing_address.email",
                        },
                    )?,
                },
            )),
        ),
        None,
        StripeBillingAddress::default(),
    )),
    payment_method_data::BankTransferData::SepaBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(StripeBankTransferData::SepaBankTransfer(
            Box::new(SepaBankTransferData {
                payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                bank_transfer_type: BankTransferType::EuBankTransfer,
                balance_funding_type: BankTransferType::BankTransfers,
                payment_method_type: StripePaymentMethodType::CustomerBalance,
                country: payment_request_details.billing_address.country.ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "billing_address.country",
                    },
                )?,
            }),
        )),
        Some(StripePaymentMethodType::CustomerBalance),
        payment_request_details.billing_address,
    )),
    payment_method_data::BankTransferData::BacsBankTransfer {} => Ok((
        StripePaymentMethodData::BankTransfer(StripeBankTransferData::BacsBankTransfers(
            Box::new(BacsBankTransferData {
                payment_method_data_type: StripePaymentMethodType::CustomerBalance,
                bank_transfer_type: BankTransferType::GbBankTransfer,
                balance_funding_type: BankTransferType::BankTransfers,
                payment_method_type: StripePaymentMethodType::CustomerBalance,
            }),
        )),
        Some(StripePaymentMethodType::CustomerBalance),
        payment_request_details.billing_address,
    )),
}
```

**Form-Encoded Request Structure:**
```
amount=10000&
currency=usd&
payment_method_data[type]=customer_balance&
payment_method_data[bank_transfer][type]=us_bank_transfer&
payment_method_options[customer_balance][funding_type]=bank_transfer&
confirm=true
```

---

### Multi-Method Request Pattern

**Applies to**: Connectors supporting multiple payment methods

**Characteristics**:
- Request Format: JSON/Form based on payment method type
- Response Type: Varies by payment method
- Amount Unit: Minor

#### Trustpay Multi-Method Pattern

```rust
// From crates/integrations/connector-integration/src/connectors/trustpay/transformers.rs:1447-1473

match item.router_data.request.payment_method_data {
    PaymentMethodData::Card(ref ccard) => Ok(get_card_request_data(
        item.router_data.clone(),
        &default_browser_info,
        params,
        amount,
        ccard,
        item.router_data.request.get_router_return_url()?,
    )?),
    PaymentMethodData::BankRedirect(ref bank_redirection_data) => {
        get_bank_redirection_request_data(
            item.router_data.clone(),
            bank_redirection_data,
            params,
            amount,
            auth,
        )
    }
    PaymentMethodData::BankTransfer(ref bank_transfer_data) => {
        get_bank_transfer_request_data(
            item.router_data.clone(),
            bank_transfer_data,
            params,
            amount,
            auth,
        )
    }
    // ... other variants
}
```

---

## Response Patterns

### Synchronous Response

Rare for Bank Transfer - most implementations require async confirmation via webhook.

### Async Response with Bank Instructions

**Applies to**: Stripe (ACH, SEPA, BACS, Multibanco)

**Response Flow:**
1. Initial response returns `requires_action` status
2. `next_action` contains bank transfer instructions
3. Customer uses instructions to complete transfer
4. Webhook confirms payment completion

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:2804-2895

fn get_next_action_response(
    next_action_response: Option<StripeNextActionResponse>,
) -> Option<NextActionsResponse> {
    next_action_response.and_then(|next_action_response| match next_action_response {
        StripeNextActionResponse::DisplayBankTransferInstructions(response) => {
            match response.financial_addresses.clone() {
                FinancialInformation::StripeFinancialInformation(financial_addresses) => {
                    let bank_instructions = financial_addresses.first();
                    let sepa_bank_instructions = bank_instructions
                        .and_then(|financial_address| {
                            financial_address
                                .payment_method_sepa_bank_account
                                .as_ref()
                                .map(|sepa| SepaFinancialDetails {
                                    account_holder_name: sepa.account_holder_name.clone(),
                                    bic: sepa.bic.clone(),
                                    iban: sepa.iban.clone(),
                                    bank_name: sepa.bank_name.clone(),
                                })
                        });
                    let bacs_bank_instructions = bank_instructions
                        .and_then(|financial_address| {
                            financial_address
                                .payment_method_bacs_debit
                                .as_ref()
                                .map(|bacs| BacsFinancialDetails {
                                    account_holder_name: bacs.account_holder_name.clone(),
                                    account_number: bacs.account_number.clone(),
                                    sort_code: bacs.sort_code.clone(),
                                })
                        });
                    let bank_transfer_instructions = SepaAndBacsBankTransferInstructions {
                        sepa_bank_instructions,
                        bacs_bank_instructions,
                        receiver: SepaAndBacsReceiver {
                            amount_received: response.amount_received,
                        },
                    };
                    Some(NextActionsResponse::BankTransferInstructions {
                        bank_transfer_instructions: BankTransferInstructions::SepaAndBacs(Box::new(
                            bank_transfer_instructions,
                        )),
                        recipient: None,
                    })
                }
                FinancialInformation::AchFinancialInformation(ach_response) => {
                    let ach_transfer_instruction = AchTransfer {
                        account_number: ach_response.account_number.clone(),
                        routing_number: ach_response.routing_number.clone(),
                        bank_name: ach_response.bank_name.clone(),
                        swift_code: ach_response.swift_code.clone(),
                        amount_charged: None,
                    };
                    let bank_transfer_instructions = BankTransferNextStepsData {
                        bank_transfer_instructions: BankTransferInstructions::AchCreditTransfer(
                            Box::new(ach_transfer_instruction),
                        ),
                        receiver: None,
                    };
                    Some(NextActionsResponse::DisplayBankTransferInstructions {
                        bank_transfer_instructions,
                    })
                }
            }
        }
        StripeNextActionResponse::MultibancoDisplayDetails(response) => {
            let multibanco_bank_transfer_instructions = BankTransferNextStepsData {
                bank_transfer_instructions: BankTransferInstructions::Multibanco(Box::new(
                    MultibancoTransferInstructions {
                        reference: response.clone().reference,
                        entity: response.clone().entity.expose(),
                    },
                )),
                receiver: None,
            };
            Some(NextActionsResponse::DisplayBankTransferInstructions {
                bank_transfer_instructions: multibanco_bank_transfer_instructions,
            })
        }
        // ... other variants
    })
}
```

**Response Structure (Stripe ACH):**
```json
{
  "id": "pi_1234567890",
  "status": "requires_action",
  "next_action": {
    "type": "display_bank_transfer_instructions",
    "display_bank_transfer_instructions": {
      "type": "us_bank_account",
      "financial_addresses": {
        "aba": {
          "account_number": "000123456789",
          "routing_number": "110000000",
          "bank_name": "Stripe Test Bank"
        }
      },
      "hosted_instructions_url": "https://pay.stripe.com/instructions/..."
    }
  }
}
```

### Redirect Response

**Applies to**: Adyen (Indonesian VA transfers)

Some bank transfers require a redirect to complete the payment flow.

```rust
// Response handling for redirect-based bank transfers
PaymentsResponseData::TransactionResponse {
    resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
    redirection_data: Some(RedirectForm::Form { ... }),
    mandate_reference: None,
    connector_metadata: None,
    network_txn_id: None,
    connector_response_reference_id: Some(reference),
    incremental_authorization_allowed: None,
    charges: None,
}
```

---

## Implementation Templates

### Macro-Based Implementation

For connectors using standard JSON API patterns:

```rust
use crate::connectors::macros::impl_api_integration;

impl_api_integration! {
    definition: ConnectorDefinition {
        name: MyConnector,
        base_url: "https://api.myconnector.com",
        auth_type: ConnectorAuthType::Body,
    },
    flows: [Authorize, Capture, Void, Refund, Psync],
    request_format: Json,
    response_format: Json,
}

impl ConnectorIntegration<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>
    for MyConnector
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<String, ConnectorError> {
        Ok(format!("{}/v1/payments", self.base_url()))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<RequestContent, ConnectorError> {
        let amount = convert_amount(
            self.amount_converter,
            req.request.minor_amount,
            req.request.currency,
        )?;

        let connector_router_data = MyConnectorRouterData::from((amount, req));
        let connector_req = MyConnectorPaymentsRequest::try_from(&connector_router_data)?;
        Ok(RequestContent::Json(Box::new(connector_req)))
    }

    fn handle_response(
        &self,
        data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>, ConnectorError> {
        let response: MyConnectorPaymentsResponse = res
            .response
            .parse_struct("MyConnectorPaymentsResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        event_builder.map(|i| i.set_response_body(&response));
        RouterDataV2::try_from(ResponseRouterData {
            response,
            data: data.clone(),
            http_code: res.status_code,
        })
    }
}
```

### Manual Implementation

For connectors requiring custom handling:

```rust
impl ConnectorIntegration<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>
    for MyConnector
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<Vec<(String, masking::Maskable<String>)>, ConnectorError> {
        let mut headers = vec![
            (CONTENT_TYPE.to_string(), "application/json".to_string().into()),
        ];

        // Add authentication headers
        let auth = MyConnectorAuthType::try_from(&req.connector_auth_type)?;
        headers.push((
            "Authorization".to_string(),
            format!("Bearer {}", auth.api_key.peek()).into(),
        ));

        Ok(headers)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<String, ConnectorError> {
        // Dynamic URL based on payment method sub-type
        let base = self.base_url();
        match req.request.payment_method_data {
            PaymentMethodData::BankTransfer(ref bt) => match bt.deref() {
                BankTransferData::AchBankTransfer => {
                    Ok(format!("{}/v1/bank-transfers/ach", base))
                }
                BankTransferData::SepaBankTransfer => {
                    Ok(format!("{}/v1/bank-transfers/sepa", base))
                }
                _ => Ok(format!("{}/v1/bank-transfers", base)),
            },
            _ => Ok(format!("{}/v1/payments", base)),
        }
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<RequestContent, ConnectorError> {
        let amount = self
            .amount_converter
            .convert(req.request.minor_amount, req.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;

        let connector_router_data = MyConnectorRouterData::from((amount, req));

        // Transform based on payment method
        let connector_req = match req.request.payment_method_data {
            PaymentMethodData::BankTransfer(_) => {
                MyConnectorBankTransferRequest::try_from(&connector_router_data)?
            }
            _ => MyConnectorPaymentRequest::try_from(&connector_router_data)?,
        };

        Ok(RequestContent::Json(Box::new(connector_req)))
    }

    fn build_request(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        connectors: &Connectors,
    ) -> CustomResult<Option<Request>, ConnectorError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Post)
                .url(&Authorize, connectors, self)
                .attach_default_headers()
                .headers(Authorize, req, connectors, self)
                .set_body(Authorize, req, connectors, self)?
                .build(),
        ))
    }

    fn handle_response(
        &self,
        data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>, ConnectorError> {
        let response: MyConnectorPaymentsResponse = res
            .response
            .parse_struct("MyConnectorPaymentsResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        event_builder.map(|i| i.set_response_body(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: MyConnectorErrorResponse = res
            .response
            .parse_struct("MyConnectorErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        event_builder.map(|i| i.set_error_response_body(&response));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code,
            message: response.message,
            reason: response.reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}
```

---

## Sub-type Variations

### Region-Based Variations

| Region | Variant | Typical Flow | Special Requirements |
|--------|---------|--------------|---------------------|
| **US** | `AchBankTransfer` | Instructions provided | Account/routing number display |
| **EU** | `SepaBankTransfer` | Instructions provided | IBAN/BIC display |
| **UK** | `BacsBankTransfer` | Instructions provided | Sort code/account number |
| **Portugal** | `MultibancoBankTransfer` | Reference + Entity | Reference number for ATM payment |
| **Indonesia** | Various VA | Redirect or QR | Virtual account number |
| **Brazil** | `Pix` | QR Code or Key | Pix key or QR code display |

### Request Structure Variations

#### ACH (US) Transfer
```rust
#[derive(Debug, Serialize)]
pub struct AchTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType, // customer_balance
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: StripeCreditTransferTypes, // us_bank_transfer
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType, // bank_transfer
    #[serde(rename = "payment_method_types[]")]
    pub payment_method_type: StripePaymentMethodType, // customer_balance
}
```

#### SEPA (EU) Transfer
```rust
#[derive(Debug, Serialize)]
pub struct SepaBankTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: BankTransferType, // eu_bank_transfer
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType,
    #[serde(rename = "payment_method_types[]")]
    pub payment_method_type: StripePaymentMethodType,
    pub country: CountryAlpha2, // Required for SEPA
}
```

#### BACS (UK) Transfer
```rust
#[derive(Debug, Serialize)]
pub struct BacsBankTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
    #[serde(rename = "payment_method_options[customer_balance][bank_transfer][type]")]
    pub bank_transfer_type: BankTransferType, // gb_bank_transfer
    #[serde(rename = "payment_method_options[customer_balance][funding_type]")]
    pub balance_funding_type: BankTransferType,
    #[serde(rename = "payment_method_types[]")]
    pub payment_method_type: StripePaymentMethodType,
}
```

#### Multibanco (Portugal) Transfer
```rust
#[derive(Debug, Serialize)]
pub struct MultibancoTransferData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripeCreditTransferTypes, // multibanco
    #[serde(rename = "payment_method_types[]")]
    pub payment_method_type: StripeCreditTransferTypes,
    pub email: Email, // Required for Multibanco
}
```

---

## Webhook Handling

Bank transfers heavily rely on webhooks for status updates since transfers are customer-initiated and can take time to complete.

### Webhook Response Mapping

```rust
impl From<WebhookStatus> for enums::AttemptStatus {
    fn from(status: WebhookStatus) -> Self {
        match status {
            WebhookStatus::Pending => Self::Pending,
            WebhookStatus::Received => Self::Authorized,
            WebhookStatus::Completed => Self::Charged,
            WebhookStatus::Failed => Self::Failure,
            WebhookStatus::Refunded => Self::CaptureMethodNotSupported,
            WebhookStatus::Disputed => Self::CaptureMethodNotSupported,
        }
    }
}
```

### Stripe Webhook Example

```rust
// Webhook event handling for bank transfers
match event.event_type {
    "payment_intent.payment_failed" => {
        // Handle failed bank transfer
        update_payment_status(
            payment_id,
            AttemptStatus::Failure,
            Some(event.data.object.last_payment_error),
        );
    }
    "payment_intent.succeeded" => {
        // Handle successful bank transfer
        update_payment_status(
            payment_id,
            AttemptStatus::Charged,
            None,
        );
    }
    "payment_intent.requires_action" => {
        // Customer needs to complete transfer
        update_payment_status(
            payment_id,
            AttemptStatus::AuthenticationPending,
            None,
        );
    }
    _ => {}
}
```

---

## Psync Behavior

### Psync Implementation for Bank Transfer

```rust
impl ConnectorIntegration<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for MyConnector
{
    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        _connectors: &Connectors,
    ) -> CustomResult<String, ConnectorError> {
        let connector_payment_id = req
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(ConnectorError::MissingConnectorTransactionID)?;

        Ok(format!("{}/v1/payments/{}", self.base_url(), connector_payment_id))
    }

    fn build_request(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        connectors: &Connectors,
    ) -> CustomResult<Option<Request>, ConnectorError> {
        Ok(Some(
            RequestBuilder::new()
                .method(Method::Get)
                .url(&PSync, connectors, self)
                .attach_default_headers()
                .headers(PSync, req, connectors, self)
                .build(),
        ))
    }

    fn handle_response(
        &self,
        data: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>, ConnectorError> {
        let response: MyConnectorSyncResponse = res
            .response
            .parse_struct("MyConnectorSyncResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        event_builder.map(|i| i.set_response_body(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            data: data.clone(),
            http_code: res.status_code,
        })
    }
}
```

### Status Mapping for Psync

```rust
impl From<MyConnectorPaymentStatus> for enums::AttemptStatus {
    fn from(status: MyConnectorPaymentStatus) -> Self {
        match status {
            MyConnectorPaymentStatus::Pending => Self::AuthenticationPending,
            MyConnectorPaymentStatus::Processing => Self::Authorized,
            MyConnectorPaymentStatus::Succeeded => Self::Charged,
            MyConnectorPaymentStatus::Failed => Self::Failure,
            MyConnectorPaymentStatus::Canceled => Self::Voided,
            MyConnectorPaymentStatus::RequiresAction => Self::AuthenticationPending,
        }
    }
}
```

---

## Common Pitfalls

### 1. Missing Required Fields

**Issue**: Bank transfer methods often require specific billing fields.

**Solution**: Always validate required fields:
```rust
// Stripe requires email for Multibanco
let email = payment_request_details.billing_address.email.ok_or(
    ConnectorError::MissingRequiredField {
        field_name: "billing_address.email",
    },
)?;

// Stripe requires country for SEPA
let country = payment_request_details.billing_address.country.ok_or(
    ConnectorError::MissingRequiredField {
        field_name: "billing_address.country",
    },
)?;
```

### 2. Webhook Dependency

**Issue**: Assuming synchronous confirmation for bank transfers.

**Solution**: Always return `AuthenticationPending` or `Authorized` status and rely on webhooks:
```rust
// WRONG - Bank transfers are not synchronous
AttemptStatus::Charged

// CORRECT - Wait for webhook
AttemptStatus::AuthenticationPending
```

### 3. Reference Number Mismatch

**Issue**: Customer uses wrong reference number for transfer.

**Solution**: Store connector reference and expose clear instructions:
```rust
let bank_transfer_instructions = BankTransferNextStepsData {
    bank_transfer_instructions: BankTransferInstructions::Multibanco(Box::new(
        MultibancoTransferInstructions {
            reference: response.reference, // This is what customer must use
            entity: response.entity,
        },
    )),
    receiver: None,
};
```

### 4. Timeout Handling

**Issue**: Bank transfers can take days to complete.

**Solution**: Implement long timeout periods and auto-cleanup:
```rust
// Configure extended timeout for bank transfers
const BANK_TRANSFER_TIMEOUT_HOURS: u64 = 72; // 3 days
```

### 5. Amount Precision

**Issue**: Transfer amount doesn't match due to currency conversion.

**Solution**: Always use minor units and include currency in instructions:
```rust
let amount_minor = req.request.minor_amount;
let currency = req.request.currency;
// Include both in customer-facing instructions
```

---

## Testing Patterns

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ach_bank_transfer_request() {
        let payment_data = BankTransferData::AchBankTransfer {};
        let router_data = create_test_router_data(payment_data);

        let result = MyConnectorPaymentRequest::try_from(&router_data);

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.payment_method_type, "us_bank_transfer");
    }

    #[test]
    fn test_sepa_bank_transfer_missing_country() {
        let payment_data = BankTransferData::SepaBankTransfer {};
        let router_data = create_test_router_data_with_missing_country(payment_data);

        let result = MyConnectorPaymentRequest::try_from(&router_data);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("billing_address.country"));
    }

    #[test]
    fn test_webhook_status_mapping() {
        assert_eq!(
            AttemptStatus::from(WebhookStatus::Received),
            AttemptStatus::Authorized
        );
        assert_eq!(
            AttemptStatus::from(WebhookStatus::Completed),
            AttemptStatus::Charged
        );
    }
}
```

### Integration Test Scenarios

| Scenario | Expected Behavior |
|----------|-------------------|
| Successful bank transfer initiation | Returns `AuthenticationPending` with instructions |
| Missing required email (Multibanco) | Returns `MissingRequiredField` error |
| Missing required country (SEPA) | Returns `MissingRequiredField` error |
| Webhook - payment received | Status updates to `Authorized` |
| Webhook - payment completed | Status updates to `Charged` |
| Webhook - payment failed | Status updates to `Failure` |
| Psync during pending state | Returns current pending status |
| Psync after completion | Returns `Charged` status |

---

## Implementation Checklist

### Pre-Implementation

- [ ] Identify which Bank Transfer variants the connector supports
- [ ] Review connector API documentation for bank transfer endpoints
- [ ] Determine request format (JSON vs FormUrlEncoded)
- [ ] Identify required fields for each variant
- [ ] Understand webhook events for status updates
- [ ] Map connector-specific status codes to `AttemptStatus`

### Implementation

- [ ] Add `BankTransfer` match arm in `get_request_body`
- [ ] Implement variant-specific request structures
- [ ] Add required field validation
- [ ] Implement response parsing with status mapping
- [ ] Implement `next_action` handling for bank instructions
- [ ] Add webhook handling for status updates
- [ ] Implement Psync for status polling

### Testing

- [ ] Unit tests for request transformation
- [ ] Unit tests for response parsing
- [ ] Unit tests for status mapping
- [ ] Integration tests with sandbox
- [ ] Webhook event testing
- [ ] End-to-end flow testing

### Documentation

- [ ] Document supported variants
- [ ] Document required fields per variant
- [ ] Document webhook events
- [ ] Provide testing credentials
- [ ] Document timeout expectations

---

## Connector-Specific Notes

### Stripe

**Key Points:**
- Uses `customer_balance` as base payment method
- Form-encoded request format
- Returns bank instructions in `next_action`
- Supports ACH, SEPA, BACS, and Multibanco

**Required Fields by Variant:**
- Multibanco: `billing_address.email`
- SEPA: `billing_address.country`
- ACH/BACS: No additional fields

### Adyen

**Key Points:**
- JSON request format
- Supports Indonesian Virtual Account transfers
- Each bank has specific payment method type
- DOKU integration for Indonesian banks

**Supported Banks:**
- Permata
- BCA
- BNI VA
- BRI VA
- CIMB VA
- Danamon VA
- Mandiri VA

### Trustpay

**Key Points:**
- Supports both JSON and FormUrlEncoded
- European bank transfers
- Requires specific browser info handling

---

## Cross-References

- [Pattern: Authorize Flow](pattern_authorize.md) - General authorize flow patterns
- [Pattern: Webhook Handling](pattern_webhook.md) - Webhook processing patterns
- [Pattern: Psync Flow](pattern_psync.md) - Status polling patterns
- [Pattern: Amount Handling](pattern_amount.md) - Amount conversion patterns
- [Connector Guide](../connector-integration-guide.md) - General connector implementation
