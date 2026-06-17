//! Stripe Connect payout transformers.

use std::fmt::Debug;

use common_utils::types::MinorUnit;
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateRecipient, PayoutEnrollDisburseAccount, PayoutGet,
        PayoutTransfer, PayoutVoid,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    payouts::payout_method_data::{Bank, PayoutMethodData},
    payouts::payouts_types::{
        PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutCreateRequest,
        PayoutCreateResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutTransferRequest, PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::report;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::stripe::StripeAmountConvertor, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================

pub struct StripeAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for StripeAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(item: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match item {
            ConnectorSpecificConfig::Stripe { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StripeConnectPayoutStatus {
    Canceled,
    Failed,
    InTransit,
    Paid,
    Pending,
}

impl From<StripeConnectPayoutStatus> for common_enums::PayoutStatus {
    fn from(status: StripeConnectPayoutStatus) -> Self {
        match status {
            StripeConnectPayoutStatus::Paid => Self::Success,
            StripeConnectPayoutStatus::Pending => Self::Pending,
            StripeConnectPayoutStatus::InTransit => Self::Pending,
            StripeConnectPayoutStatus::Failed => Self::Failure,
            StripeConnectPayoutStatus::Canceled => Self::Cancelled,
        }
    }
}

fn stripe_currency_string(currency: common_enums::Currency) -> String {
    currency.to_string().to_lowercase()
}

const STRIPE_ACCOUNT_TYPE_INDIVIDUAL: &str = "individual";
const STRIPE_ACCOUNT_TYPE_COMPANY: &str = "company";
const STRIPE_EXTERNAL_ACCOUNT_OBJECT_BANK: &str = "bank_account";

fn tos_acceptance_now() -> Result<i64, error_stack::Report<IntegrationError>> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| IntegrationError::InvalidDataFormat {
            field_name: "system_time",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "System clock is set before the Unix epoch, so tos_acceptance[date] cannot be computed"
                        .to_string(),
                ),
                suggested_action: Some("Ensure the host system clock is configured correctly".to_string()),
                doc_url: None,
            },
        })?
        .as_secs();
    i64::try_from(secs).map_err(|_| {
        error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name: "system_time",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Current Unix timestamp does not fit into i64, so tos_acceptance[date] cannot be computed"
                        .to_string(),
                ),
                suggested_action: Some("Ensure the host system clock is configured correctly".to_string()),
                doc_url: None,
            },
        })
    })
}

// =============================================================================
// PAYOUT CREATE (TRANSFER CREATE)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct StripeConnectPayoutCreateRequest {
    pub amount: MinorUnit,

    pub currency: String,

    pub destination: String,
    #[serde(rename = "transfer_group")]
    pub transfer_group: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::StripePayoutsRouterData<
            RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
            T,
        >,
    > for StripeConnectPayoutCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::StripePayoutsRouterData<
            RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let amount = StripeAmountConvertor::convert(request.amount, request.source_currency)?;
        let currency = stripe_currency_string(request.source_currency);

        let destination = request.connector_payout_method_id.clone().ok_or_else(|| {
            report!(IntegrationError::MissingRequiredField {
                field_name: "connector_payout_method_id",
                context: Default::default(),
            })
        })?;

        let transfer_group = Some(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        );

        Ok(Self {
            amount,
            currency,
            destination,
            transfer_group,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectPayoutCreateResponse {
    pub id: String,
}

impl TryFrom<ResponseRouterData<StripeConnectPayoutCreateResponse, Self>>
    for RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectPayoutCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutCreateResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::RequiresFulfillment,
                connector_payout_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// PAYOUT FULFILL (PAYOUT CREATE)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct StripeConnectPayoutFulfillRequest {
    pub amount: MinorUnit,

    pub currency: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for StripeConnectPayoutFulfillRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let amount = StripeAmountConvertor::convert(request.amount, request.source_currency)?;
        let currency = stripe_currency_string(request.source_currency);
        Ok(Self { amount, currency })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectPayoutFulfillResponse {
    pub id: String,

    pub status: StripeConnectPayoutStatus,
}

impl TryFrom<ResponseRouterData<StripeConnectPayoutFulfillResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectPayoutFulfillResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: item.response.status.into(),
                connector_payout_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// PAYOUT VOID (TRANSFER REVERSAL)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct StripeConnectReversalRequest {
    pub amount: Option<MinorUnit>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectReversalResponse {
    pub id: String,

    #[serde(rename = "source_refund")]
    pub source_refund: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::StripePayoutsRouterData<
            RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
            T,
        >,
    > for StripeConnectReversalRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::StripePayoutsRouterData<
            RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self { amount: None })
    }
}

impl TryFrom<ResponseRouterData<StripeConnectReversalResponse, Self>>
    for RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectReversalResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutVoidResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::Cancelled,
                connector_payout_id: item.router_data.request.connector_payout_id.clone(),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// PAYOUT GET (PAYOUT RETRIEVE)
// =============================================================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectPayoutRetrieveResponse {
    pub id: String,

    pub amount: MinorUnit,

    pub currency: String,

    pub status: StripeConnectPayoutStatus,

    pub description: Option<String>,

    #[serde(rename = "failure_code")]
    pub failure_code: Option<String>,

    #[serde(rename = "failure_message")]
    pub failure_message: Option<String>,
}

impl TryFrom<ResponseRouterData<StripeConnectPayoutRetrieveResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectPayoutRetrieveResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutGetResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: item.response.status.into(),
                connector_payout_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// RECIPIENT CREATE (CONNECTED ACCOUNT)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct StripeConnectRecipientCreateRequest {
    #[serde(rename = "type")]
    pub account_type: String,

    pub country: Option<common_enums::CountryAlpha2>,

    pub email: Option<common_utils::pii::Email>,

    #[serde(rename = "capabilities[card_payments][requested]")]
    pub capabilities_card_payments: Option<bool>,

    #[serde(rename = "capabilities[transfers][requested]")]
    pub capabilities_transfers: Option<bool>,

    #[serde(rename = "tos_acceptance[date]")]
    pub tos_acceptance_date: Option<i64>,

    #[serde(rename = "tos_acceptance[ip]")]
    pub tos_acceptance_ip: Option<Secret<String>>,

    #[serde(rename = "business_type")]
    pub business_type: String,

    #[serde(rename = "business_profile[mcc]")]
    pub business_profile_mcc: Option<i32>,

    #[serde(rename = "business_profile[url]")]
    pub business_profile_url: Option<Secret<String>>,

    #[serde(rename = "business_profile[name]")]
    pub business_profile_name: Option<Secret<String>>,

    #[serde(rename = "company[name]")]
    pub company_name: Option<Secret<String>>,

    #[serde(rename = "company[address][line1]")]
    pub company_address_line1: Option<Secret<String>>,

    #[serde(rename = "company[address][line2]")]
    pub company_address_line2: Option<Secret<String>>,

    #[serde(rename = "company[address][postal_code]")]
    pub company_address_postal_code: Option<Secret<String>>,

    #[serde(rename = "company[address][city]")]
    pub company_address_city: Option<Secret<String>>,

    #[serde(rename = "company[address][state]")]
    pub company_address_state: Option<Secret<String>>,

    #[serde(rename = "company[phone]")]
    pub company_phone: Option<Secret<String>>,

    #[serde(rename = "company[tax_id]")]
    pub company_tax_id: Option<Secret<String>>,

    #[serde(rename = "company[owners_provided]")]
    pub company_owners_provided: Option<bool>,

    #[serde(rename = "individual[first_name]")]
    pub individual_first_name: Option<Secret<String>>,

    #[serde(rename = "individual[last_name]")]
    pub individual_last_name: Option<Secret<String>>,

    #[serde(rename = "individual[dob][day]")]
    pub individual_dob_day: Option<Secret<String>>,

    #[serde(rename = "individual[dob][month]")]
    pub individual_dob_month: Option<Secret<String>>,

    #[serde(rename = "individual[dob][year]")]
    pub individual_dob_year: Option<Secret<String>>,

    #[serde(rename = "individual[address][line1]")]
    pub individual_address_line1: Option<Secret<String>>,

    #[serde(rename = "individual[address][line2]")]
    pub individual_address_line2: Option<Secret<String>>,

    #[serde(rename = "individual[address][postal_code]")]
    pub individual_address_postal_code: Option<Secret<String>>,

    #[serde(rename = "individual[address][city]")]
    pub individual_address_city: Option<Secret<String>>,

    #[serde(rename = "individual[address][state]")]
    pub individual_address_state: Option<Secret<String>>,

    #[serde(rename = "individual[email]")]
    pub individual_email: Option<common_utils::pii::Email>,

    #[serde(rename = "individual[phone]")]
    pub individual_phone: Option<Secret<String>>,

    #[serde(rename = "individual[id_number]")]
    pub individual_id_number: Option<Secret<String>>,

    #[serde(rename = "individual[ssn_last_4]")]
    pub individual_ssn_last_4: Option<Secret<String>>,

    #[serde(rename = "settings[payments][statement_descriptor]")]
    pub statement_descriptor: Option<Secret<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectRecipientCreateResponse {
    pub id: String,
}

// =============================================================================
// RECIPIENT ACCOUNT CREATE (EXTERNAL ACCOUNT)
// =============================================================================

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum StripeConnectRecipientAccountCreateRequest {
    Bank(RecipientBankAccountRequest),

    Token(RecipientTokenRequest),
}

#[derive(Clone, Debug, Serialize)]
pub struct RecipientBankAccountRequest {
    #[serde(rename = "external_account[object]")]
    pub external_account_object: String,

    #[serde(rename = "external_account[country]")]
    pub external_account_country: common_enums::CountryAlpha2,

    #[serde(rename = "external_account[currency]")]
    pub external_account_currency: String,

    #[serde(rename = "external_account[account_holder_name]")]
    pub external_account_account_holder_name: Secret<String>,

    #[serde(rename = "external_account[account_holder_type]")]
    pub external_account_account_holder_type: String,

    #[serde(rename = "external_account[account_number]")]
    pub external_account_account_number: Secret<String>,

    #[serde(rename = "external_account[routing_number]")]
    pub external_account_routing_number: Secret<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecipientTokenRequest {
    #[serde(rename = "external_account")]
    pub external_account: Secret<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectRecipientAccountCreateResponse {
    pub id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    > for StripeConnectRecipientCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        let is_company = request.is_company();
        let business_type = if is_company {
            STRIPE_ACCOUNT_TYPE_COMPANY
        } else {
            STRIPE_ACCOUNT_TYPE_INDIVIDUAL
        }
        .to_string();

        let account_type = request.get_account_type()?;
        let phone = request.get_phone()?;
        let first_name = request.get_first_name()?;
        let last_name = request.get_last_name()?;
        let dob_day = request.get_dob_day()?;
        let dob_month = request.get_dob_month()?;
        let dob_year = request.get_dob_year()?;
        let business_profile_mcc = request.get_business_profile_mcc_i32()?;
        let business_profile_url = request.get_business_profile_url()?;
        let business_profile_name = request.get_business_profile_name()?;
        let statement_descriptor = request.get_statement_descriptor()?;
        let tos_acceptance_ip = request.get_tos_acceptance_ip()?;
        let (id_number, ssn_last_4) = request.get_id_number_or_ssn_last_4()?;

        let email = request.get_email_with_fallback();
        let addr_line1 = request.get_optional_billing_line1();
        let addr_line2 = request.get_optional_billing_line2();
        let addr_zip = request.get_optional_billing_zip();
        let addr_city = request.get_optional_billing_city();
        let addr_state = request.get_optional_billing_state();
        let addr_country = request.get_optional_billing_country().ok_or_else(|| {
            report!(IntegrationError::MissingRequiredField {
                field_name: "billing.address.country",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Billing country is required to create a Stripe connected account"
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Provide the recipient's billing address country".to_string(),
                    ),
                    doc_url: None,
                },
            })
        })?;

        Ok(Self {
            account_type,
            country: Some(addr_country),
            email: email.clone(),
            capabilities_card_payments: Some(true),
            capabilities_transfers: Some(true),
            tos_acceptance_date: Some(tos_acceptance_now()?),
            tos_acceptance_ip: Some(tos_acceptance_ip),
            business_type,
            business_profile_mcc: Some(business_profile_mcc),
            business_profile_url: Some(business_profile_url),
            business_profile_name: Some(business_profile_name.clone()),

            company_name: is_company.then_some(business_profile_name),
            company_address_line1: addr_line1.clone().filter(|_| is_company),
            company_address_line2: addr_line2.clone().filter(|_| is_company),
            company_address_postal_code: addr_zip.clone().filter(|_| is_company),
            company_address_city: addr_city.clone().filter(|_| is_company),
            company_address_state: addr_state.clone().filter(|_| is_company),
            company_phone: is_company.then(|| phone.clone()),
            company_tax_id: id_number.clone().filter(|_| is_company),
            company_owners_provided: None,

            individual_first_name: (!is_company).then_some(first_name),
            individual_last_name: (!is_company).then_some(last_name),
            individual_dob_day: (!is_company).then_some(dob_day),
            individual_dob_month: (!is_company).then_some(dob_month),
            individual_dob_year: (!is_company).then_some(dob_year),
            individual_address_line1: addr_line1.filter(|_| !is_company),
            individual_address_line2: addr_line2.filter(|_| !is_company),
            individual_address_postal_code: addr_zip.filter(|_| !is_company),
            individual_address_city: addr_city.filter(|_| !is_company),
            individual_address_state: addr_state.filter(|_| !is_company),
            individual_email: email,
            individual_phone: (!is_company).then_some(phone),
            individual_id_number: id_number.filter(|_| !is_company),
            individual_ssn_last_4: ssn_last_4.filter(|_| !is_company),

            statement_descriptor: Some(statement_descriptor),
        })
    }
}

impl TryFrom<ResponseRouterData<StripeConnectRecipientCreateResponse, Self>>
    for RouterDataV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectRecipientCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutCreateRecipientResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::RequiresCreation,
                connector_payout_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutEnrollDisburseAccount,
                PayoutFlowData,
                PayoutEnrollDisburseAccountRequest,
                PayoutEnrollDisburseAccountResponse,
            >,
            T,
        >,
    > for StripeConnectRecipientAccountCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::StripePayoutsRouterData<
            RouterDataV2<
                PayoutEnrollDisburseAccount,
                PayoutFlowData,
                PayoutEnrollDisburseAccountRequest,
                PayoutEnrollDisburseAccountResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let payout_method_data = request.get_payout_method_data()?;

        match payout_method_data {
            PayoutMethodData::Bank(Bank::Ach(ach)) => {
                let country = ach.bank_country_code.ok_or_else(|| {
                    report!(IntegrationError::MissingRequiredField {
                        field_name: "bank_country_code",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Bank country code is required to create a Stripe external bank account"
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Provide the bank account country code".to_string(),
                            ),
                            doc_url: None,
                        },
                    })
                })?;
                let currency = stripe_currency_string(request.source_currency);
                let account_holder_name = request.get_customer_name().ok_or_else(|| {
                    report!(IntegrationError::MissingRequiredField {
                        field_name: "customer.name",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Account holder name is required to create a Stripe external bank account"
                                    .to_string(),
                            ),
                            suggested_action: Some("Provide the customer name".to_string()),
                            doc_url: None,
                        },
                    })
                })?;

                Ok(Self::Bank(RecipientBankAccountRequest {
                    external_account_object: STRIPE_EXTERNAL_ACCOUNT_OBJECT_BANK.to_string(),
                    external_account_country: country,
                    external_account_currency: currency,
                    external_account_account_holder_name: account_holder_name,
                    external_account_account_holder_type: STRIPE_ACCOUNT_TYPE_INDIVIDUAL.to_string(),
                    external_account_account_number: ach.bank_account_number.clone(),
                    external_account_routing_number: ach.bank_routing_number.clone(),
                }))
            }
            _ => Err(IntegrationError::NotImplemented(
                "Only ACH bank transfers are supported for external account enrollment".to_string(),
                IntegrationErrorContext::default(),
            )
            .into()),
        }
    }
}

impl TryFrom<ResponseRouterData<StripeConnectRecipientAccountCreateResponse, Self>>
    for RouterDataV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<StripeConnectRecipientAccountCreateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutEnrollDisburseAccountResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::RequiresCreation,
                connector_payout_id: Some(item.response.id.clone()),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// =============================================================================
// ERROR RESPONSE
// =============================================================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectErrorResponse {
    pub error: StripeConnectError,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StripeConnectError {
    pub code: Option<String>,

    pub message: String,

    pub decline_code: Option<String>,
}
