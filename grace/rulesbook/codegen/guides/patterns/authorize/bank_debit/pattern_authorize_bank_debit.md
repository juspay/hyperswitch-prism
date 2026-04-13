# Bank Debit Authorize Flow Pattern for Grace-UCS Connectors

**Payment Method**: Bank Debit (ACH, SEPA, BECS, BACS)
**Pattern Type**: Direct Debit / Mandate-based
**Last Updated**: 2026-02-19

## Table of Contents

1. [Overview](#overview)
2. [Bank Debit Variants](#bank-debit-variants)
3. [Supported Connectors](#supported-connectors)
4. [Quick Reference](#quick-reference)
5. [Request Patterns](#request-patterns)
6. [Response Patterns](#response-patterns)
7. [Implementation Templates](#implementation-templates)
8. [Mandate Handling](#mandate-handling)
9. [Sub-type Variations](#sub-type-variations)
10. [Common Pitfalls](#common-pitfalls)
11. [Testing Patterns](#testing-patterns)
12. [Implementation Checklist](#implementation-checklist)

## Overview

Bank Debit is a payment method that allows merchants to collect payments directly from a customer's bank account. Unlike card payments, bank debits typically require:

- **Mandate Authorization**: Customer consent for recurring debits
- **Account Details**: Bank account numbers, routing codes, or IBANs
- **Delayed Settlement**: Processing times of 3-7 business days
- **Reversible Transactions**: Higher chargeback risk than cards

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Payment Flow** | Asynchronous with delayed confirmation |
| **Mandate Required** | Yes, for recurring payments |
| **Settlement Time** | 3-7 business days |
| **Chargeback Window** | 8 weeks (SEPA), 60 days (ACH) |
| **Authentication** | Customer bank credentials or mandate acceptance |

## Bank Debit Variants

The system supports four primary bank debit variants defined in `payment_method_data.rs`:

```rust
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BankDebitData {
    AchBankDebit {
        account_number: Secret<String>,
        routing_number: Secret<String>,
        card_holder_name: Option<Secret<String>>,
        bank_account_holder_name: Option<Secret<String>>,
        bank_name: Option<common_enums::BankNames>,
        bank_type: Option<common_enums::BankType>,
        bank_holder_type: Option<common_enums::BankHolderType>,
    },
    SepaBankDebit {
        iban: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BecsBankDebit {
        account_number: Secret<String>,
        bsb_number: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
    BacsBankDebit {
        account_number: Secret<String>,
        sort_code: Secret<String>,
        bank_account_holder_name: Option<Secret<String>>,
    },
}
```

### Variant Details

| Variant | Region | Key Fields | Use Case |
|---------|--------|------------|----------|
| **AchBankDebit** | USA | `account_number`, `routing_number` | US bank account debits |
| **SepaBankDebit** | EU | `iban` | Single Euro Payments Area |
| **BecsBankDebit** | Australia | `account_number`, `bsb_number` | Australian bank debits |
| **BacsBankDebit** | UK | `account_number`, `sort_code` | UK direct debits |

## Supported Connectors

| Connector | ACH | SEPA | BECS | BACS | Mandate Support | Notes |
|-----------|-----|------|------|------|-----------------|-------|
| **Adyen** | ✅ | ✅ | ❌ | ✅ | ✅ | Full mandate support |
| **Stripe** | ✅ | ✅ | ✅ | ✅ | ✅ | Requires mandate_data for recurring |
| **Novalnet** | ❌ | ✅ | ❌ | ❌ | ✅ | SEPA only |
| **PayPal** | ❌ | ❌ | ❌ | ❌ | ❌ | Not implemented |
| **Worldpay** | ❌ | ❌ | ❌ | ❌ | ❌ | Not implemented |

## Quick Reference

### Bank Debit Data Extraction Pattern

```rust
use domain_types::payment_method_data::{BankDebitData, PaymentMethodData};

pub fn extract_bank_debit_data<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<&BankDebitData, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::BankDebit(bank_debit_data) => Ok(bank_debit_data),
        _ => Err(IntegrationError::NotImplemented(
            "Only Bank Debit payments are supported".to_string(, Default::default())
        )),
    }
}
```

### Account Holder Name Extraction

```rust
pub fn get_account_holder_name(
    bank_debit_data: &BankDebitData,
    router_data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<Secret<String>, IntegrationError> {
    match bank_debit_data {
        BankDebitData::AchBankDebit { bank_account_holder_name, .. }
        | BankDebitData::SepaBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BecsBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BacsBankDebit { bank_account_holder_name, .. } => {
            bank_account_holder_name
                .clone()
                .or_else(|| router_data.resource_common_data.get_billing_full_name().ok())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "bank_account_holder_name",
                , context: Default::default() }.into())
        }
    }
}
```

## Request Patterns

### Standard JSON Pattern (Adyen)

**Applies to**: Adyen

**Characteristics**:
- Request Format: JSON
- Response Type: Async with webhook confirmation
- Amount Unit: MinorUnit
- Mandate: Required for recurring payments

```rust
// File: crates/integrations/connector-integration/src/connectors/adyen/transformers.rs

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &BankDebitData,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(
        (bank_debit_data, item): (
            &BankDebitData,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        match bank_debit_data {
            BankDebitData::AchBankDebit {
                account_number,
                routing_number,
                ..
            } => Ok(Self::AchDirectDebit(Box::new(AchDirectDebitData {
                bank_account_number: account_number.clone(),
                bank_location_id: routing_number.clone(),
                owner_name: item.resource_common_data.get_billing_full_name()?,
            }))),
            BankDebitData::SepaBankDebit { iban, .. } => {
                Ok(Self::SepaDirectDebit(Box::new(SepaDirectDebitData {
                    owner_name: item.resource_common_data.get_billing_full_name()?,
                    iban_number: iban.clone(),
                })))
            }
            BankDebitData::BacsBankDebit {
                account_number,
                sort_code,
                ..
            } => {
                let testing_data = item
                    .request
                    .get_connector_testing_data()
                    .map(AdyenTestingData::try_from)
                    .transpose()?;
                let test_holder_name = testing_data.and_then(|test_data| test_data.holder_name);
                Ok(Self::BacsDirectDebit(Box::new(BacsDirectDebitData {
                    bank_account_number: account_number.clone(),
                    bank_location_id: sort_code.clone(),
                    holder_name: test_holder_name
                        .unwrap_or(item.resource_common_data.get_billing_full_name()?),
                })))
            }
            BankDebitData::BecsBankDebit { .. } => Err(errors::IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen", Default::default()),
            )
            .into()),
        }
    }
}
```

### ACH-Specific State Code Handling

```rust
// For ACH bank debit, override state_or_province with state code
let billing_address = match bank_debit_data {
    BankDebitData::AchBankDebit { .. } => billing_address.map(|mut addr| {
        addr.state_or_province = item
            .router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|b| b.address.as_ref())
            .and_then(|address| address.to_state_code_as_optional().ok().flatten())
            .or(addr.state_or_province);
        addr
    }),
    BankDebitData::SepaBankDebit { .. }
    | BankDebitData::BacsBankDebit { .. }
    | BankDebitData::BecsBankDebit { .. } => billing_address,
};
```

### Stripe Bank Debit Pattern

**Applies to**: Stripe

**Characteristics**:
- Request Format: JSON
- Response Type: Async
- Amount Unit: MinorUnit
- Special: Requires mandate_data for recurring payments

```rust
// File: crates/integrations/connector-integration/src/connectors/stripe/transformers.rs

impl From<&payment_method_data::BankDebitData> for StripePaymentMethodType {
    fn from(bank_debit_data: &payment_method_data::BankDebitData) -> Self {
        match bank_debit_data {
            payment_method_data::BankDebitData::AchBankDebit { .. } => Self::Ach,
            payment_method_data::BankDebitData::SepaBankDebit { .. } => Self::Sepa,
            payment_method_data::BankDebitData::BecsBankDebit { .. } => Self::Becs,
            payment_method_data::BankDebitData::BacsBankDebit { .. } => Self::Bacs,
        }
    }
}

fn get_bank_debit_data(
    bank_debit_data: &payment_method_data::BankDebitData,
) -> (StripePaymentMethodType, BankDebitData) {
    match bank_debit_data {
        payment_method_data::BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            ..
        } => {
            let ach_data = BankDebitData::Ach {
                account_holder_type: "individual".to_string(),
                account_number: account_number.to_owned(),
                routing_number: routing_number.to_owned(),
            };
            (StripePaymentMethodType::Ach, ach_data)
        }
        payment_method_data::BankDebitData::SepaBankDebit { iban, .. } => {
            let sepa_data: BankDebitData = BankDebitData::Sepa {
                iban: iban.to_owned(),
            };
            (StripePaymentMethodType::Sepa, sepa_data)
        }
        payment_method_data::BankDebitData::BecsBankDebit {
            account_number,
            bsb_number,
            ..
        } => {
            let becs_data = BankDebitData::Becs {
                account_number: account_number.to_owned(),
                bsb_number: bsb_number.to_owned(),
            };
            (StripePaymentMethodType::Becs, becs_data)
        }
        payment_method_data::BankDebitData::BacsBankDebit {
            account_number,
            sort_code,
            ..
        } => {
            let bacs_data = BankDebitData::Bacs {
                account_number: account_number.to_owned(),
                sort_code: Secret::new(sort_code.clone().expose().replace('-', "")),
            };
            (StripePaymentMethodType::Bacs, bacs_data)
        }
    }
}
```

### SEPA-Only Pattern (Novalnet)

**Applies to**: Novalnet

**Characteristics**:
- Request Format: JSON
- Response Type: Redirect/Async
- Amount Unit: StringMinorUnit
- Limited: SEPA only

```rust
// File: crates/integrations/connector-integration/src/connectors/novalnet/transformers.rs

PaymentMethodData::BankDebit(ref bank_debit_data) => {
    let payment_type = NovalNetPaymentTypes::try_from(
        &item
            .router_data
            .request
            .payment_method_type
            .ok_or(IntegrationError::MissingPaymentMethodType)?,
    )?;

    let (iban, account_holder) = match bank_debit_data {
        BankDebitData::SepaBankDebit {
            iban,
            bank_account_holder_name,
        } => {
            let account_holder = match bank_account_holder_name {
                Some(name) => name.clone(),
                None => item
                    .router_data
                    .resource_common_data
                    .get_billing_full_name()?,
            };
            (iban.clone(), account_holder)
        }
        BankDebitData::AchBankDebit { .. }
        | BankDebitData::BecsBankDebit { .. }
        | BankDebitData::BacsBankDebit { .. } => {
            return Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("novalnet", Default::default()),
            )
            .into());
        }
    };

    let transaction = NovalnetPaymentsRequestTransaction {
        test_mode,
        payment_type,
        amount: NovalNetAmount::StringMinor(amount.clone()),
        currency: item.router_data.request.currency,
        order_no: item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        hook_url: Some(hook_url),
        return_url: Some(return_url.clone()),
        error_return_url: Some(return_url.clone()),
        payment_data: Some(NovalNetPaymentData::Sepa(NovalnetSepaDebit {
            account_holder: account_holder.clone(),
            iban,
        })),
        enforce_3d,
        create_token,
    };
    // ...
}
```

## Response Patterns

### Bank Debit Status Mapping

Bank debit responses typically follow an asynchronous pattern:

```rust
// Standard bank debit status mapping
impl From<ConnectorBankDebitStatus> for common_enums::AttemptStatus {
    fn from(status: ConnectorBankDebitStatus) -> Self {
        match status {
            ConnectorBankDebitStatus::Pending => Self::Pending,
            ConnectorBankDebitStatus::Processing => Self::Pending,
            ConnectorBankDebitStatus::Confirmed => Self::Charged,
            ConnectorBankDebitStatus::Failed => Self::Failure,
            ConnectorBankDebitStatus::Cancelled => Self::Voided,
            ConnectorBankDebitStatus::Refunded => Self::Refunded,
            ConnectorBankDebitStatus::Chargeback => Self::Chargeback,
        }
    }
}
```

### Response Handling with Mandate Reference

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<BankDebitAuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BankDebitAuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        let mandate_reference = response.mandate_id.as_ref().map(|mandate_id| MandateReference {
            connector_mandate_id: Some(mandate_id.clone()),
            payment_method_id: None,
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
            redirection_data: response.redirect_url.as_ref().map(|url| {
                RedirectForm::Uri { uri: url.clone() }
            }),
            mandate_reference,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.reference.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
```

## Implementation Templates

### Complete Bank Debit Connector Implementation

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Accept, Authorize, Capture, CreateOrder, CreateSessionToken, DefendDispute, PSync, RSync, Refund, RepeatPayment, SetupMandate, SubmitEvidence, Void},
    connector_types::{AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData, PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData, SubmitEvidenceData},
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types, events::connector_api_logs::ConnectorEvent};
use serde::Serialize;
use transformers::{ConnectorNameAuthorizeRequest, ConnectorNameAuthorizeResponse, ConnectorNameErrorResponse, ConnectorNameSyncRequest, ConnectorNameSyncResponse};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for ConnectorName<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for ConnectorName<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for ConnectorName<T>
{
}

macros::create_all_prerequisites!(
    connector_name: ConnectorName,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ConnectorNameAuthorizeRequest<T>,
            response_body: ConnectorNameAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: ConnectorNameSyncRequest,
            response_body: ConnectorNameSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.connector_name.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.connector_name.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    ConnectorCommon for ConnectorName<T>
{
    fn id(&self) -> &'static str {
        "connector_name"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.connector_name.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::ConnectorNameAuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: ConnectorNameErrorResponse = if res.response.is_empty() {
            ConnectorNameErrorResponse::default()
        } else {
            res.response
                .parse_struct("ErrorResponse")
                .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?
        };

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status: None,
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ConnectorName,
    curl_request: Json(ConnectorNameAuthorizeRequest),
    curl_response: ConnectorNameAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/v1/payments"))
        }
    }
);

use interfaces::verification::SourceVerification;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
    for ConnectorName<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for ConnectorName<T>
{
}
```

### Transformers File Template

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use common_utils::{ext_traits::OptionExt, pii, request::Method, types::MinorUnit};
use domain_types::{
    connector_flow::{self, Authorize, PSync},
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId},
    errors::{self, IntegrationError},
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret, PeekInterface};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// Authentication Type Definition
#[derive(Debug)]
pub struct ConnectorNameAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for ConnectorNameAuthType {
    type Error = IntegrationError;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// Bank Debit Request Structure
#[derive(Debug, Serialize)]
pub struct ConnectorNameBankDebitRequest {
    pub account_number: Secret<String>,
    pub routing_number: Option<Secret<String>>,
    pub iban: Option<Secret<String>>,
    pub sort_code: Option<Secret<String>>,
    pub bsb_number: Option<Secret<String>>,
    pub account_holder_name: Secret<String>,
    #[serde(rename = "type")]
    pub bank_debit_type: BankDebitType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BankDebitType {
    Ach,
    Sepa,
    Becs,
    Bacs,
}

impl From<&BankDebitData> for BankDebitType {
    fn from(data: &BankDebitData) -> Self {
        match data {
            BankDebitData::AchBankDebit { .. } => Self::Ach,
            BankDebitData::SepaBankDebit { .. } => Self::Sepa,
            BankDebitData::BecsBankDebit { .. } => Self::Becs,
            BankDebitData::BacsBankDebit { .. } => Self::Bacs,
        }
    }
}

// Main Authorize Request
#[derive(Debug, Serialize)]
pub struct ConnectorNameAuthorizeRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub amount: MinorUnit,
    pub currency: String,
    pub payment_method: ConnectorNamePaymentMethod,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mandate_data: Option<ConnectorNameMandateData>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConnectorNamePaymentMethod {
    BankDebit(ConnectorNameBankDebitRequest),
}

// Response Structure
#[derive(Debug, Deserialize)]
pub struct ConnectorNameAuthorizeResponse {
    pub id: String,
    pub status: ConnectorNamePaymentStatus,
    pub amount: Option<i64>,
    pub reference: Option<String>,
    pub mandate_id: Option<String>,
    pub redirect_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorNamePaymentStatus {
    Pending,
    Processing,
    Confirmed,
    Failed,
    Cancelled,
}

impl From<ConnectorNamePaymentStatus> for common_enums::AttemptStatus {
    fn from(status: ConnectorNamePaymentStatus) -> Self {
        match status {
            ConnectorNamePaymentStatus::Pending | ConnectorNamePaymentStatus::Processing => {
                Self::Pending
            }
            ConnectorNamePaymentStatus::Confirmed => Self::Charged,
            ConnectorNamePaymentStatus::Failed => Self::Failure,
            ConnectorNamePaymentStatus::Cancelled => Self::Voided,
        }
    }
}

// Error Response
#[derive(Debug, Deserialize, Default)]
pub struct ConnectorNameErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

// Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<ConnectorNameRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for ConnectorNameAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ConnectorNameRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::BankDebit(bank_debit_data) => {
                let bank_debit_request = create_bank_debit_request(
                    bank_debit_data,
                    router_data,
                )?;
                ConnectorNamePaymentMethod::BankDebit(bank_debit_request)
            }
            _ => return Err(IntegrationError::NotImplemented(
                "Only Bank Debit payments are supported".to_string(, Default::default())
            ).into()),
        };

        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
            mandate_data: None, // Populate if mandate is required
        })
    }
}

fn create_bank_debit_request<T: PaymentMethodDataTypes>(
    bank_debit_data: &BankDebitData,
    router_data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<ConnectorNameBankDebitRequest, IntegrationError> {
    let account_holder_name = get_account_holder_name(bank_debit_data, router_data)?;
    let bank_debit_type = BankDebitType::from(bank_debit_data);

    match bank_debit_data {
        BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            ..
        } => Ok(ConnectorNameBankDebitRequest {
            account_number: account_number.clone(),
            routing_number: Some(routing_number.clone()),
            iban: None,
            sort_code: None,
            bsb_number: None,
            account_holder_name,
            bank_debit_type,
        }),
        BankDebitData::SepaBankDebit { iban, .. } => Ok(ConnectorNameBankDebitRequest {
            account_number: iban.clone(),
            routing_number: None,
            iban: Some(iban.clone()),
            sort_code: None,
            bsb_number: None,
            account_holder_name,
            bank_debit_type,
        }),
        BankDebitData::BecsBankDebit {
            account_number,
            bsb_number,
            ..
        } => Ok(ConnectorNameBankDebitRequest {
            account_number: account_number.clone(),
            routing_number: None,
            iban: None,
            sort_code: None,
            bsb_number: Some(bsb_number.clone()),
            account_holder_name,
            bank_debit_type,
        }),
        BankDebitData::BacsBankDebit {
            account_number,
            sort_code,
            ..
        } => Ok(ConnectorNameBankDebitRequest {
            account_number: account_number.clone(),
            routing_number: None,
            iban: None,
            sort_code: Some(sort_code.clone()),
            bsb_number: None,
            account_holder_name,
            bank_debit_type,
        }),
    }
}

fn get_account_holder_name<T: PaymentMethodDataTypes>(
    bank_debit_data: &BankDebitData,
    router_data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Result<Secret<String>, IntegrationError> {
    match bank_debit_data {
        BankDebitData::AchBankDebit { bank_account_holder_name, .. }
        | BankDebitData::SepaBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BecsBankDebit { bank_account_holder_name, .. }
        | BankDebitData::BacsBankDebit { bank_account_holder_name, .. } => {
            bank_account_holder_name
                .clone()
                .or_else(|| router_data.resource_common_data.get_billing_full_name().ok())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "bank_account_holder_name",
                , context: Default::default() })
        }
    }
}

// Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ConnectorNameAuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ConnectorNameAuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        let mandate_reference = response.mandate_id.as_ref().map(|id| domain_types::connector_types::MandateReference {
            connector_mandate_id: Some(id.clone()),
            payment_method_id: None,
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: response.redirect_url.as_ref().map(|url| {
                RedirectForm::Uri { uri: url.clone() }
            }),
            mandate_reference,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: response.reference.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Router Data Helper
pub struct ConnectorNameRouterData<T, U> {
    pub amount: MinorUnit,
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(MinorUnit, T, U)> for ConnectorNameRouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((amount, router_data, connector): (MinorUnit, T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data,
            connector,
        })
    }
}
```

## Mandate Handling

Bank debits typically require mandate handling for recurring payments. Here's the pattern:

### Mandate Data Structure

```rust
#[derive(Debug, Serialize)]
pub struct ConnectorNameMandateData {
    pub mandate_type: MandateType,
    pub reference: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MandateType {
    SingleUse,
    MultiUse,
}
```

### Stripe Mandate Pattern

```rust
// For recurring payments with saved bank debit
.or_else(|| {
    // Stripe requires mandate_data for recurring payments through saved bank debit
    if payment_method.is_some() {
        // Check if payment is done through saved payment method
        match &payment_method_types {
            // Check if payment method is bank debit
            Some(
                StripePaymentMethodType::Ach
                | StripePaymentMethodType::Sepa
                | StripePaymentMethodType::Becs
                | StripePaymentMethodType::Bacs,
            ) => Some(StripeMandateRequest {
                mandate_type: StripeMandateType::Offline,
            }),
            _ => None,
        }
    } else {
        None
    }
});
```

### Adyen Mandate Support

```rust
// In Adyen, mandate support is checked during setup mandate flow
let mandate_supported_pmd = std::collections::HashSet::from([
    PaymentMethodDataType::Card,
    PaymentMethodDataType::AchBankDebit,
    PaymentMethodDataType::SepaBankDebit,
    PaymentMethodDataType::BecsBankDebit,
]);
is_mandate_supported(pm_data, pm_type, mandate_supported_pmd, self.id())
```

## Sub-type Variations

| Sub-type | Region | Account Identifier | Routing Identifier | Holder Name Source | Special Handling |
|----------|--------|-------------------|-------------------|-------------------|------------------|
| **AchBankDebit** | USA | `account_number` | `routing_number` | `bank_account_holder_name` or billing name | State code conversion |
| **SepaBankDebit** | EU | `iban` | N/A | `bank_account_holder_name` or billing name | IBAN validation |
| **BecsBankDebit** | Australia | `account_number` | `bsb_number` | `bank_account_holder_name` or billing name | BSB formatting |
| **BacsBankDebit** | UK | `account_number` | `sort_code` | `bank_account_holder_name` or billing name | Sort code formatting |

### Regional-Specific Handling

#### ACH State Code Handling (USA)

```rust
// For ACH bank debit, override state_or_province with state code
let billing_address = match bank_debit_data {
    BankDebitData::AchBankDebit { .. } => billing_address.map(|mut addr| {
        addr.state_or_province = item
            .router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|b| b.address.as_ref())
            .and_then(|address| address.to_state_code_as_optional().ok().flatten())
            .or(addr.state_or_province);
        addr
    }),
    _ => billing_address,
};
```

#### BACS Sort Code Formatting (UK)

```rust
// Remove hyphens from sort code for BACS
BankDebitData::BacsBankDebit { sort_code, .. } => {
    let formatted_sort_code = Secret::new(
        sort_code.clone().expose().replace('-', "")
    );
    // Use formatted_sort_code
}
```

## Common Pitfalls

### 1. Missing Account Holder Name

**Problem**: Bank debit requires an account holder name, but it's optional in the request.

**Solution**: Fall back to billing name if not provided:

```rust
let account_holder = match bank_account_holder_name {
    Some(name) => name.clone(),
    None => item
        .router_data
        .resource_common_data
        .get_billing_full_name()?,
};
```

### 2. Not Implementing All Sub-types

**Problem**: Connector only supports a subset of bank debit variants.

**Solution**: Explicitly return NotImplemented for unsupported variants:

```rust
BankDebitData::BecsBankDebit { .. } => {
    Err(IntegrationError::NotImplemented(
        utils::get_unimplemented_payment_method_error_message("ConnectorName", Default::default()),
    )
    .into())
}
```

### 3. Missing Mandate for Recurring Payments

**Problem**: Bank debits require mandates for recurring payments, but this is not enforced.

**Solution**: Check for mandate requirement and populate mandate_data:

```rust
// In request transformation
pub mandate_data: Option<ConnectorNameMandateData>,

// Populate based on payment context
mandate_data: if router_data.request.setup_mandate_required {
    Some(create_mandate_data(router_data)?)
} else {
    None
},
```

### 4. Incorrect Status Mapping

**Problem**: Bank debits often return "Pending" status initially, but this is not mapped correctly.

**Solution**: Map connector statuses accurately:

```rust
impl From<ConnectorBankDebitStatus> for common_enums::AttemptStatus {
    fn from(status: ConnectorBankDebitStatus) -> Self {
        match status {
            ConnectorBankDebitStatus::Pending => Self::Pending,
            ConnectorBankDebitStatus::Processing => Self::Pending,
            ConnectorBankDebitStatus::Confirmed => Self::Charged,
            ConnectorBankDebitStatus::Failed => Self::Failure,
            // Don't assume Charged for pending statuses!
        }
    }
}
```

### 5. Synchronous Response Assumption

**Problem**: Treating bank debit as synchronous when it's actually asynchronous.

**Solution**: Always return Pending for initial bank debit responses and rely on webhooks for final status.

## Testing Patterns

### Unit Test for Bank Debit Request Transformation

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ach_bank_debit_request() {
        let bank_debit_data = BankDebitData::AchBankDebit {
            account_number: Secret::new("000123456789".to_string()),
            routing_number: Secret::new("110000000".to_string()),
            bank_account_holder_name: Some(Secret::new("John Doe".to_string())),
            card_holder_name: None,
            bank_name: None,
            bank_type: None,
            bank_holder_type: None,
        };

        let request = create_bank_debit_request(
            &bank_debit_data,
            &create_test_router_data(),
        ).unwrap();

        assert_eq!(request.bank_debit_type, BankDebitType::Ach);
        assert!(request.routing_number.is_some());
        assert_eq!(request.iban, None);
    }

    #[test]
    fn test_sepa_bank_debit_request() {
        let bank_debit_data = BankDebitData::SepaBankDebit {
            iban: Secret::new("DE89370400440532013000".to_string()),
            bank_account_holder_name: Some(Secret::new("Jane Doe".to_string())),
        };

        let request = create_bank_debit_request(
            &bank_debit_data,
            &create_test_router_data(),
        ).unwrap();

        assert_eq!(request.bank_debit_type, BankDebitType::Sepa);
        assert!(request.iban.is_some());
        assert_eq!(request.routing_number, None);
    }

    #[test]
    fn test_unsupported_bank_debit() {
        let bank_debit_data = BankDebitData::BecsBankDebit {
            account_number: Secret::new("000123456".to_string()),
            bsb_number: Secret::new("062-000".to_string()),
            bank_account_holder_name: None,
        };

        let result = create_bank_debit_request(
            &bank_debit_data,
            &create_test_router_data(),
        );

        // If BECS is not supported
        assert!(result.is_err());
    }
}
```

### Integration Test for Mandate Handling

```rust
#[tokio::test]
async fn test_bank_debit_mandate_creation() {
    let connector = ConnectorName::new();

    // Test mandate creation for recurring payment
    let request_data = create_test_authorize_request_with_mandate();

    let request_body = connector.get_request_body(&request_data).unwrap();
    assert!(request_body.is_some());

    // Verify mandate data is present
    if let Some(RequestContent::Json(body)) = request_body {
        let json_str = serde_json::to_string(&body).unwrap();
        assert!(json_str.contains("mandate"));
    }
}
```

## Implementation Checklist

### Pre-Implementation

- [ ] Review connector's bank debit API documentation
- [ ] Identify supported bank debit variants (ACH, SEPA, BECS, BACS)
- [ ] Understand mandate requirements and flow
- [ ] Confirm async webhook handling capability
- [ ] Identify regional-specific requirements (state codes, IBAN format, etc.)

### Implementation

- [ ] Create bank debit request structure with all variant fields
- [ ] Implement match arms for all BankDebitData variants
- [ ] Add account holder name extraction with fallback
- [ ] Implement regional-specific handling (state codes, formatting)
- [ ] Add mandate data population for recurring payments
- [ ] Implement proper status mapping (Pending for async)
- [ ] Handle unsupported variants with NotImplemented error

### Testing

- [ ] Unit test for each bank debit variant
- [ ] Test account holder name extraction
- [ ] Test mandate data generation
- [ ] Test status mapping for all statuses
- [ ] Test error handling for unsupported variants
- [ ] Integration test with sandbox credentials

### Validation

- [ ] Verify all BankDebitData variants are handled
- [ ] Confirm mandate reference is extracted in response
- [ ] Validate async webhook handling
- [ ] Test with real bank account credentials in sandbox
- [ ] Verify chargeback handling

---

## Cross-References

- [Pattern Authorize General](pattern_authorize.md) - Generic authorize flow patterns
- [Amount Patterns](../amount_patterns.md) - Amount conversion patterns
- [Auth Patterns](../auth_patterns.md) - Authentication patterns
- [Utility Functions Reference](../utility_functions_reference.md) - Common helper functions

## Connector-Specific References

- **Adyen**: `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1654-1696`
- **Stripe**: `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:1204-1260`
- **Novalnet**: `crates/integrations/connector-integration/src/connectors/novalnet/transformers.rs:467-527`
