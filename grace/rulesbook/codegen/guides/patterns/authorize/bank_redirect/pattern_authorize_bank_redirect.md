# Bank Redirect Authorize Flow Patterns

## Overview

Bank Redirect is a payment method category that enables customers to complete payments by redirecting to their bank's online banking interface. This payment method is widely used in Europe and other regions where customers prefer direct bank transfers over cards.

### Key Characteristics

- **Customer Experience**: Customer is redirected to their bank's website to authenticate and authorize the payment
- **Async Nature**: Most bank redirects are asynchronous - final status comes via webhook or psync
- **Regional Variations**: Different bank redirect methods exist for different countries/regions
- **No Card Data**: No card numbers or sensitive payment data is handled by the merchant

### Bank Redirect Variants

| Variant | Description | Common Regions |
|---------|-------------|----------------|
| `BancontactCard` | Belgian payment system | Belgium |
| `Bizum` | Spanish mobile payment | Spain |
| `Blik` | Polish mobile payment | Poland |
| `Eft` | Electronic Funds Transfer | South Africa |
| `Eps` | Austrian payment standard | Austria |
| `Giropay` | German online banking | Germany |
| `Ideal` | Dutch payment system | Netherlands |
| `Interac` | Canadian debit system | Canada |
| `OnlineBankingCzechRepublic` | Czech bank transfers | Czech Republic |
| `OnlineBankingFinland` | Finnish bank transfers | Finland |
| `OnlineBankingPoland` | Polish bank transfers | Poland |
| `OnlineBankingSlovakia` | Slovak bank transfers | Slovakia |
| `OpenBanking` | Generic Open Banking EU | EU |
| `OpenBankingUk` | UK Open Banking | UK |
| `Przelewy24` | Polish payment system | Poland |
| `Sofort` | Instant bank transfer | Germany, Austria, Switzerland |
| `Trustly` | European bank transfers | Europe |
| `OnlineBankingFpx` | Malaysian FPX | Malaysia |
| `OnlineBankingThailand` | Thai bank transfers | Thailand |
| `LocalBankRedirect` | Generic local redirect | Various |

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Supported Connectors](#supported-connectors)
3. [Request Patterns](#request-patterns)
4. [Response Patterns](#response-patterns)
5. [Implementation Templates](#implementation-templates)
6. [Sub-type Variations](#sub-type-variations)
7. [Common Pitfalls](#common-pitfalls)
8. [Testing Patterns](#testing-patterns)

## Quick Reference

### Pattern Summary Table

| Pattern | Request Format | Response Type | Amount Unit | Connectors |
|---------|---------------|---------------|-------------|------------|
| Standard JSON | JSON | Async/Redirect | StringMajorUnit | Adyen, Mollie, MultiSafepay, Nexinets |
| Form-Encoded | FormUrlEncoded | Async/Redirect | StringMajorUnit | Trustpay (Cards), Adyen (Some types) |
| Dynamic Content Type | JSON/Form | Async/Redirect | StringMajorUnit | Trustpay |
| Access Token Required | JSON | Async/Redirect | StringMajorUnit | Trustpay, Volt |
| Open Banking Specific | JSON | Async/Redirect | MinorUnit | Volt, Stripe |

## Supported Connectors

| Connector | Request Format | Auth Method | Sub-types Supported | Webhook Support |
|-----------|---------------|-------------|---------------------|-----------------|
| **Adyen** | JSON | API Key | eps, giropay, ideal, sofort, trustly | Yes |
| **Stripe** | JSON | API Key | ideal, sofort, bancontact, giropay, p24, eps | Yes |
| **Trustpay** | Dynamic (JSON/Form) | API Key + OAuth | OpenBanking, OpenBankingUk | Yes |
| **Mollie** | JSON | API Key | ideal, sofort, bancontact, eps, giropay, p24 | Yes |
| **Volt** | JSON | OAuth | OpenBanking, OpenBankingUk | Yes |
| **MultiSafepay** | JSON | API Key | ideal, sofort, bancontact, eps, giropay, p24 | Yes |
| **Nexinets** | JSON | OAuth | giropay, ideal, p24 | Yes |
| **PayPal** | JSON | OAuth | Various | Yes |
| **Gigadat** | FormUrlEncoded | API Key | Interac | Yes |
| **IataPay** | JSON | API Key | OpenBanking | Yes |
| **Airwallex** | JSON | API Key | Various | Yes |
| **GlobalPay** | JSON | API Key | Various | Yes |
| **Shift4** | JSON | API Key | Various | Yes |
| **Fiuu** | JSON | API Key | Various | Yes |
| **ACI** | JSON | API Key | Various | Yes |

## Request Patterns

### Pattern 1: Standard JSON Request

**Applies to**: Adyen, Mollie, MultiSafepay, Nexinets

**Characteristics**:
- Request Format: JSON
- Authentication: API Key in headers
- Amount Unit: StringMajorUnit (converted from minor units)
- Content-Type: `application/json`

#### Implementation Template

```rust
#[derive(Debug, Serialize)]
pub struct BankRedirectPaymentRequest {
    amount: Amount,
    currency: Currency,
    payment_method: PaymentMethodDetails,
    return_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Amount {
    currency: Currency,
    value: StringMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum PaymentMethodDetails {
    #[serde(rename = "ideal")]
    Ideal { issuer: String },
    #[serde(rename = "sofort")]
    Sofort { country: String },
    #[serde(rename = "giropay")]
    Giropay { bic: Option<String> },
    #[serde(rename = "eps")]
    Eps { bank: String },
    #[serde(rename = "bancontact")]
    Bancontact,
    #[serde(rename = "p24")]
    Przelewy24 { bank: String },
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &ConnectorRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BankRedirectPaymentRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &ConnectorRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(item.router_data.request.minor_amount, item.router_data.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;

        let payment_method = match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(bank_redirect) => match bank_redirect {
                BankRedirectData::Ideal { bank_name, .. } => PaymentMethodDetails::Ideal {
                    issuer: bank_name.to_string(),
                },
                BankRedirectData::Sofort { .. } => PaymentMethodDetails::Sofort {
                    country: "DE".to_string(), // or derive from billing address
                },
                BankRedirectData::Giropay { .. } => PaymentMethodDetails::Giropay { bic: None },
                BankRedirectData::Eps { bank_name, .. } => PaymentMethodDetails::Eps {
                    bank: bank_name.to_string(),
                },
                BankRedirectData::BancontactCard { .. } => PaymentMethodDetails::Bancontact,
                BankRedirectData::Przelewy24 { bank_name, .. } => PaymentMethodDetails::Przelewy24 {
                    bank: bank_name.to_string(),
                },
                _ => Err(ConnectorError::NotImplemented(
                    "Bank redirect type not supported".to_string(),
                ))?,
            },
            _ => Err(ConnectorError::NotImplemented(
                "Payment method not supported".to_string(),
            ))?,
        };

        Ok(Self {
            amount: Amount {
                currency: item.router_data.request.currency,
                value: amount,
            },
            currency: item.router_data.request.currency,
            payment_method,
            return_url: item.router_data.request.router_return_url.clone(),
            description: Some(format!(
                "Payment for order {}",
                item.router_data.resource_common_data.connector_request_reference_id
            )),
        })
    }
}
```

### Pattern 2: Dynamic Content Type (Trustpay)

**Applies to**: Trustpay

**Characteristics**:
- Request Format: Dynamic (FormUrlEncoded for Cards, JSON for BankRedirect)
- Authentication: API Key for Cards, OAuth Bearer token for BankRedirect
- Amount Unit: StringMajorUnit
- Different base URLs for different payment methods

#### Implementation Template

```rust
// Content type selector implementation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ContentTypeSelector<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
    for Trustpay<T>
{
    fn get_dynamic_content_type(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<DynamicContentType, errors::ConnectorError> {
        match req.resource_common_data.payment_method {
            PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => {
                Ok(DynamicContentType::Json)
            }
            _ => Ok(DynamicContentType::FormUrlEncoded),
        }
    }
}

// Dynamic header building
pub fn build_headers_for_payments<F, Req, Res>(
    &self,
    req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
where
    Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
{
    match req.resource_common_data.payment_method {
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => {
            let token = req
                .resource_common_data
                .get_access_token()
                .change_context(errors::ConnectorError::MissingRequiredField {
                    field_name: "access_token",
                })?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_owned().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {token}").into_masked(),
                ),
            ])
        }
        _ => {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut api_key);
            Ok(header)
        }
    }
}

// Dynamic URL selection
fn get_url(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<String, errors::ConnectorError> {
    match req.resource_common_data.payment_method {
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => Ok(format!(
            "{}{}",
            self.connector_base_url_bank_redirects_payments(req),
            "api/Payments/Payment"
        )),
        _ => Ok(format!(
            "{}{}",
            self.connector_base_url_payments(req),
            "api/v1/purchase"
        )),
    }
}
```

### Pattern 3: Open Banking Specific (Volt)

**Applies to**: Volt

**Characteristics**:
- Request Format: JSON
- Authentication: OAuth 2.0 (password grant type)
- Amount Unit: MinorUnit (no conversion needed)
- Region-specific payment systems (OpenBankingUk vs OpenBankingEu)

#### Implementation Template

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltPaymentsRequest {
    amount: MinorUnit,
    currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_banking_u_k: Option<OpenBankingUk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_banking_e_u: Option<OpenBankingEu>,
    internal_reference: String,
    payer: PayerDetails,
    payment_system: PaymentSystem,
    communication: CommunicationDetails,
}

#[derive(Debug, Serialize)]
pub struct OpenBankingUk {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
}

#[derive(Debug, Serialize)]
pub struct OpenBankingEu {
    #[serde(rename = "type")]
    transaction_type: TransactionType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentSystem {
    OpenBankingEu,
    OpenBankingUk,
    NppPayToAu,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayerDetails {
    reference: CustomerId,
    email: Option<Email>,
    first_name: Secret<String>,
    last_name: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &VoltRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for VoltPaymentsRequest
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: VoltRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(ref bank_redirect) => {
                let transaction_type = TransactionType::Services;
                let currency = item.router_data.request.currency;

                let (payment_system, open_banking_u_k, open_banking_e_u) = match bank_redirect {
                    BankRedirectData::OpenBankingUk { .. } => Ok((
                        PaymentSystem::OpenBankingUk,
                        Some(OpenBankingUk { transaction_type }),
                        None,
                    )),
                    BankRedirectData::OpenBanking {} => {
                        if matches!(currency, Currency::GBP) {
                            Ok((
                                PaymentSystem::OpenBankingUk,
                                Some(OpenBankingUk { transaction_type }),
                                None,
                            ))
                        } else {
                            Ok((
                                PaymentSystem::OpenBankingEu,
                                None,
                                Some(OpenBankingEu { transaction_type }),
                            ))
                        }
                    }
                    _ => Err(errors::ConnectorError::NotImplemented(
                        "Bank redirect type not supported".to_string(),
                    )),
                }?;

                let address = item
                    .router_data
                    .resource_common_data
                    .get_billing_address()?;
                let first_name = address.get_first_name()?;

                Ok(Self {
                    amount: item.router_data.request.amount,
                    currency,
                    internal_reference: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    payer: PayerDetails {
                        email: item.router_data.request.get_optional_email(),
                        first_name: first_name.to_owned(),
                        last_name: address.get_last_name().unwrap_or(first_name).to_owned(),
                        reference: item
                            .router_data
                            .resource_common_data
                            .get_customer_id()?
                            .to_owned(),
                    },
                    payment_system,
                    open_banking_u_k,
                    open_banking_e_u,
                    communication: CommunicationDetails {
                        return_urls: ReturnUrls {
                            success: Link {
                                link: item.router_data.request.router_return_url.clone(),
                            },
                            failure: Link {
                                link: item.router_data.request.router_return_url.clone(),
                            },
                            pending: Link {
                                link: item.router_data.request.router_return_url.clone(),
                            },
                            cancel: Link {
                                link: item.router_data.request.router_return_url.clone(),
                            },
                        },
                    },
                })
            }
            _ => Err(errors::ConnectorError::NotImplemented(
                "Payment method not supported".to_string(),
            )),
        }
    }
}
```

## Response Patterns

### Pattern 1: Redirect Response

Most Bank Redirect connectors return a redirect URL that the customer must navigate to complete the payment.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankRedirectResponse {
    id: String,
    status: BankRedirectStatus,
    redirect_url: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BankRedirectStatus {
    Pending,
    Redirect,
    Processing,
    Completed,
    Failed,
}

impl<F, T> TryFrom<ResponseRouterData<BankRedirectResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<BankRedirectResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            BankRedirectStatus::Pending | BankRedirectStatus::Processing => AttemptStatus::Pending,
            BankRedirectStatus::Redirect => AttemptStatus::AuthenticationPending,
            BankRedirectStatus::Completed => AttemptStatus::Charged,
            BankRedirectStatus::Failed => AttemptStatus::Failure,
        };

        let redirection_data = if status == AttemptStatus::AuthenticationPending {
            Some(RedirectForm::Form {
                endpoint: item.response.redirect_url.expose(),
                method: Method::Get,
                form_fields: Default::default(),
            })
        } else {
            None
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
```

### Pattern 2: Async Status Mapping

Bank redirect payments often have complex status mappings due to their asynchronous nature.

```rust
#[derive(Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BankRedirectPaymentStatus {
    NewPayment,
    ApprovedByRisk,
    AdditionalAuthorizationRequired,
    AuthorisedByUser,
    ProviderCommunicationError,
    Completed,
    Received,
    NotReceived,
    BankRedirect,
    DelayedAtBank,
    AwaitingCheckoutAuthorisation,
    RefusedByBank,
    RefusedByRisk,
    ErrorAtBank,
    CancelledByUser,
    AbandonedByUser,
    Failed,
    Settled,
    Unknown,
}

fn get_attempt_status(item: BankRedirectPaymentStatus) -> AttemptStatus {
    match item {
        BankRedirectPaymentStatus::Received | BankRedirectPaymentStatus::Settled => {
            AttemptStatus::Charged
        }
        BankRedirectPaymentStatus::Completed
        | BankRedirectPaymentStatus::DelayedAtBank
        | BankRedirectPaymentStatus::AuthorisedByUser
        | BankRedirectPaymentStatus::ApprovedByRisk => AttemptStatus::Pending,
        BankRedirectPaymentStatus::NewPayment
        | BankRedirectPaymentStatus::BankRedirect
        | BankRedirectPaymentStatus::AwaitingCheckoutAuthorisation
        | BankRedirectPaymentStatus::AdditionalAuthorizationRequired => {
            AttemptStatus::AuthenticationPending
        }
        BankRedirectPaymentStatus::RefusedByBank
        | BankRedirectPaymentStatus::RefusedByRisk
        | BankRedirectPaymentStatus::NotReceived
        | BankRedirectPaymentStatus::ErrorAtBank
        | BankRedirectPaymentStatus::CancelledByUser
        | BankRedirectPaymentStatus::AbandonedByUser
        | BankRedirectPaymentStatus::Failed
        | BankRedirectPaymentStatus::ProviderCommunicationError => AttemptStatus::Failure,
        BankRedirectPaymentStatus::Unknown => AttemptStatus::Unspecified,
    }
}
```

## Implementation Templates

### Macro-Based Implementation

```rust
// Define request and response types
#[derive(Debug, Serialize)]
pub struct ConnectorBankRedirectRequest {
    // Request fields
}

#[derive(Debug, Deserialize)]
pub struct ConnectorBankRedirectResponse {
    // Response fields
}

// Create prerequisites
macros::create_all_prerequisites!(
    connector_name: YourConnector,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ConnectorBankRedirectRequest,
            response_body: ConnectorBankRedirectResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
);

// Implement the authorize flow
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: YourConnector,
    curl_request: Json(ConnectorBankRedirectRequest),
    curl_response: ConnectorBankRedirectResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::ConnectorError> {
            Ok(format!(
                "{}/payments",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);
```

### Manual Implementation with OAuth

For connectors requiring OAuth tokens before payment:

```rust
// Validation trait to trigger access token flow
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourConnector<T>
{
    fn should_do_access_token(&self, payment_method: PaymentMethod) -> bool {
        matches!(
            payment_method,
            PaymentMethod::BankRedirect | PaymentMethod::BankTransfer
        )
    }
}

// Access token request
#[derive(Debug, Clone, Serialize)]
pub struct ConnectorAuthUpdateRequest {
    grant_type: String,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    username: Secret<String>,
    password: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for ConnectorAuthUpdateRequest {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        let auth = ConnectorAuthType::try_from(auth_type)?;
        Ok(Self {
            grant_type: "password".to_string(),
            username: auth.username,
            password: auth.password,
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

// Access token response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectorAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
}

impl<F, T> TryFrom<ResponseRouterData<ConnectorAuthUpdateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, AccessTokenResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ConnectorAuthUpdateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(AccessTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}
```

## Sub-type Variations

### Regional Bank Redirects

| Sub-type | Region | Issuer/Bank Required | Special Fields |
|----------|--------|---------------------|----------------|
| `Ideal` | Netherlands | Yes | `bank_name` - Required |
| `Sofort` | Germany/Austria | No | `country` - Optional |
| `Giropay` | Germany | No | `bic` - Optional |
| `Eps` | Austria | Yes | `bank_name` - Required |
| `BancontactCard` | Belgium | No | `card_number` - Optional |
| `Przelewy24` | Poland | Yes | `bank_name` - Required |
| `Interac` | Canada | No | `email` - Required |
| `Blik` | Poland | No | `blik_code` - Required |

### Implementation Pattern for Sub-types

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &ConnectorRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ConnectorPaymentsRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &ConnectorRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let payment_method_data = match &item.router_data.request.payment_method_data {
            PaymentMethodData::BankRedirect(bank_redirect) => {
                Self::get_bank_redirect_request(bank_redirect, item)?
            }
            _ => Err(ConnectorError::NotImplemented(
                "Payment method not supported".to_string(),
            ))?,
        };

        Ok(Self {
            // ... other fields
            payment_method: payment_method_data,
        })
    }
}

impl ConnectorPaymentsRequest {
    fn get_bank_redirect_request<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
        bank_redirect: &BankRedirectData,
        item: &ConnectorRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<PaymentMethodDetails, ConnectorError> {
        match bank_redirect {
            BankRedirectData::Ideal { bank_name, .. } => {
                Ok(PaymentMethodDetails::Ideal {
                    issuer: bank_name.to_string(),
                })
            }
            BankRedirectData::Sofort { .. } => {
                let country = item
                    .router_data
                    .resource_common_data
                    .get_optional_billing()
                    .and_then(|billing| billing.address.as_ref())
                    .and_then(|address| address.country)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "DE".to_string());
                Ok(PaymentMethodDetails::Sofort { country })
            }
            BankRedirectData::Giropay { .. } => {
                Ok(PaymentMethodDetails::Giropay { bic: None })
            }
            BankRedirectData::Eps { bank_name, .. } => {
                Ok(PaymentMethodDetails::Eps {
                    bank: bank_name.to_string(),
                })
            }
            BankRedirectData::BancontactCard { .. } => {
                Ok(PaymentMethodDetails::Bancontact)
            }
            BankRedirectData::Przelewy24 { bank_name, .. } => {
                Ok(PaymentMethodDetails::Przelewy24 {
                    bank: bank_name.to_string(),
                })
            }
            BankRedirectData::Blik { blik_code, .. } => {
                Ok(PaymentMethodDetails::Blik {
                    code: blik_code.to_string(),
                })
            }
            BankRedirectData::Interac { .. } => {
                let email = item
                    .router_data
                    .request
                    .get_optional_email()
                    .ok_or(ConnectorError::MissingRequiredField {
                        field_name: "email",
                    })?;
                Ok(PaymentMethodDetails::Interac { email })
            }
            _ => Err(ConnectorError::NotImplemented(
                "Bank redirect type not supported".to_string(),
            )),
        }
    }
}
```

## Common Pitfalls

### 1. Missing Access Token

**Problem**: Bank redirect connectors often require OAuth tokens, but the token might not be present.

**Solution**:
```rust
fn should_do_access_token(&self, payment_method: PaymentMethod) -> bool {
    matches!(
        payment_method,
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer
    )
}
```

### 2. Incorrect Content Type

**Problem**: Some connectors like Trustpay use different content types for different payment methods.

**Solution**:
```rust
fn get_dynamic_content_type(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<DynamicContentType, errors::ConnectorError> {
    match req.resource_common_data.payment_method {
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => {
            Ok(DynamicContentType::Json)
        }
        _ => Ok(DynamicContentType::FormUrlEncoded),
    }
}
```

### 3. Wrong Base URL

**Problem**: Bank redirect APIs often use different base URLs than card APIs.

**Solution**:
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<String, errors::ConnectorError> {
    match req.resource_common_data.payment_method {
        PaymentMethod::BankRedirect | PaymentMethod::BankTransfer => Ok(format!(
            "{}{}",
            self.connector_base_url_bank_redirects_payments(req),
            "api/Payments/Payment"
        )),
        _ => Ok(format!(
            "{}{}",
            self.connector_base_url_payments(req),
            "api/v1/purchase"
        )),
    }
}
```

### 4. Missing Return URLs

**Problem**: Bank redirects require return URLs for customer redirection.

**Solution**:
```rust
let communication = CommunicationDetails {
    return_urls: ReturnUrls {
        success: Link {
            link: item.router_data.request.router_return_url.clone(),
        },
        failure: Link {
            link: item.router_data.request.router_return_url.clone(),
        },
        pending: Link {
            link: item.router_data.request.router_return_url.clone(),
        },
        cancel: Link {
            link: item.router_data.request.router_return_url.clone(),
        },
    },
};
```

### 5. Status Mapping Complexity

**Problem**: Bank redirect statuses are complex and can have multiple intermediate states.

**Solution**: Create a comprehensive status mapping function:
```rust
fn get_attempt_status(item: BankRedirectPaymentStatus) -> AttemptStatus {
    match item {
        BankRedirectPaymentStatus::Received | BankRedirectPaymentStatus::Settled => {
            AttemptStatus::Charged
        }
        BankRedirectPaymentStatus::Completed
        | BankRedirectPaymentStatus::DelayedAtBank
        | BankRedirectPaymentStatus::AuthorisedByUser
        | BankRedirectPaymentStatus::ApprovedByRisk => AttemptStatus::Pending,
        BankRedirectPaymentStatus::NewPayment
        | BankRedirectPaymentStatus::BankRedirect
        | BankRedirectPaymentStatus::AwaitingCheckoutAuthorisation
        | BankRedirectPaymentStatus::AdditionalAuthorizationRequired => {
            AttemptStatus::AuthenticationPending
        }
        BankRedirectPaymentStatus::RefusedByBank
        | BankRedirectPaymentStatus::RefusedByRisk
        | BankRedirectPaymentStatus::NotReceived
        | BankRedirectPaymentStatus::ErrorAtBank
        | BankRedirectPaymentStatus::CancelledByUser
        | BankRedirectPaymentStatus::AbandonedByUser
        | BankRedirectPaymentStatus::Failed
        | BankRedirectPaymentStatus::ProviderCommunicationError => AttemptStatus::Failure,
        BankRedirectPaymentStatus::Unknown => AttemptStatus::Unspecified,
    }
}
```

## Testing Patterns

### Unit Tests for Request Building

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ideal_request_building() {
        let bank_redirect = BankRedirectData::Ideal {
            bank_name: "ING".to_string(),
        };

        let request = PaymentMethodDetails::try_from(&bank_redirect).unwrap();

        match request {
            PaymentMethodDetails::Ideal { issuer } => {
                assert_eq!(issuer, "ING");
            }
            _ => panic!("Expected Ideal payment method"),
        }
    }

    #[test]
    fn test_sofort_request_building() {
        let bank_redirect = BankRedirectData::Sofort;

        let request = PaymentMethodDetails::try_from(&bank_redirect).unwrap();

        match request {
            PaymentMethodDetails::Sofort { country } => {
                assert_eq!(country, "DE");
            }
            _ => panic!("Expected Sofort payment method"),
        }
    }
}
```

### Integration Tests for Status Mapping

```rust
#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn test_received_status_maps_to_charged() {
        let status = BankRedirectPaymentStatus::Received;
        assert_eq!(get_attempt_status(status), AttemptStatus::Charged);
    }

    #[test]
    fn test_pending_status_maps_to_pending() {
        let status = BankRedirectPaymentStatus::Completed;
        assert_eq!(get_attempt_status(status), AttemptStatus::Pending);
    }

    #[test]
    fn test_failed_status_maps_to_failure() {
        let status = BankRedirectPaymentStatus::Failed;
        assert_eq!(get_attempt_status(status), AttemptStatus::Failure);
    }
}
```

## Implementation Checklist

When implementing a new connector with Bank Redirect support:

- [ ] Identify supported bank redirect sub-types
- [ ] Determine if OAuth access token is required
- [ ] Configure dynamic content type if needed (e.g., Trustpay)
- [ ] Set up different base URLs if bank redirect uses separate endpoint
- [ ] Implement request type matching for each sub-type
- [ ] Implement response status mapping
- [ ] Configure webhook handling for async updates
- [ ] Test each sub-type individually
- [ ] Verify redirect URL construction
- [ ] Test error scenarios

## Cross-References

- [pattern_authorize.md](./pattern_authorize.md) - General authorize flow patterns
- [pattern_webhooks.md](./pattern_webhooks.md) - Webhook handling patterns
- [pattern_psync.md](./pattern_psync.md) - Payment sync patterns for async payments

---

**Document Version**: 1.0
**Last Updated**: 2026-02-19
**Maintained By**: Grace-UCS Connector Team
