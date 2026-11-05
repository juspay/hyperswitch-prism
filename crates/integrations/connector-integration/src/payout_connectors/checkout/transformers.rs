use super::CheckoutPayoutsRouterData;
use crate::types::ResponseRouterData;
use common_enums::PayoutStatus;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{PayoutGet, PayoutTransfer},
    errors::{ConnectorError, IntegrationError},
    payment_address::{Address, AddressDetails},
    payment_method_data::PaymentMethodDataTypes,
    payouts::{
        payout_method_data::{PayoutMethodData, Wallet},
        payouts_types::{
            PayoutFlowData, PayoutGetRequest, PayoutGetResponse, PayoutIndividualDetails,
            PayoutSourceOfFunds, PayoutTransferRequest, PayoutTransferResponse,
            PayoutVendorDetails,
        },
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    utils::missing_field_err,
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Checkout payout credentials resolved from the connector config.
pub struct CheckoutPayoutCredentials<'a> {
    pub api_key: &'a Secret<String>,
    pub api_secret: &'a Secret<String>,
    pub processing_channel_id: &'a Secret<String>,
    pub client_id: Option<&'a Secret<String>>,
}

pub fn credentials(
    config: &ConnectorSpecificConfig,
) -> Result<CheckoutPayoutCredentials<'_>, error_stack::Report<IntegrationError>> {
    match config {
        ConnectorSpecificConfig::Checkout {
            api_key,
            api_secret,
            processing_channel_id,
            client_id,
            ..
        } => Ok(CheckoutPayoutCredentials {
            api_key,
            api_secret,
            processing_channel_id,
            client_id: client_id.as_ref(),
        }),
        _ => Err(missing_field_err("checkout.connector_config")()),
    }
}

#[derive(Debug, Serialize)]
pub struct CheckoutPayoutRequest {
    source: CheckoutPayoutSource,
    destination: CheckoutPayoutMethod,
    amount: MinorUnit,
    currency: common_enums::Currency,
    reference: String,
    processing_channel_id: Secret<String>,
    instruction: CheckoutPayoutInstruction,
    sender: CheckoutPayoutSender,
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CheckoutPayoutSource {
    CurrencyAccount(CheckoutPayoutCurrencyAccount),
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutCurrencyAccount {
    id: Secret<String>,
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CheckoutPayoutMethod {
    NetworkToken(Box<CheckoutPayoutNetworkToken>),
    #[serde(rename = "id")]
    PaymentInstrument(Box<CheckoutPaymentInstrument>),
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutNetworkToken {
    token: cards::CardNumber,
    expiry_month: Secret<u8>,
    expiry_year: Secret<u16>,
    token_type: CheckoutPayoutTokenType,
    // Optional until cryptogram and ECI are supplied by the shared payout contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    cryptogram: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eci: Option<String>,
    account_holder: CheckoutPayoutAccountHolder,
}
#[derive(Debug, Serialize)]
struct CheckoutPaymentInstrument {
    id: Secret<String>,
    account_holder: CheckoutPayoutAccountHolder,
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutAccountHolder {
    #[serde(rename = "type")]
    holder_type: CheckoutPayoutAccountHolderType,
    first_name: Secret<String>,
    last_name: Secret<String>,
    billing_address: CheckoutPayoutAddress,
}
#[derive(Debug, Clone, Serialize)]
struct CheckoutPayoutAddress {
    address_line1: Secret<String>,
    city: Secret<String>,
    country: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<Secret<String>>,
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutInstruction {
    // Checkout assigns this value. Absence must serialize as null.
    funds_transfer_type:
        Option<domain_types::payouts::payouts_types::CheckoutPayoutFundsTransferType>,
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CheckoutPayoutSender {
    Individual(CheckoutPayoutIndividualSender),
    Corporate(CheckoutPayoutCorporateSender),
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutIndividualSender {
    first_name: Secret<String>,
    last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_of_birth: Option<Secret<String>>,
    address: CheckoutPayoutAddress,
    reference: CheckoutPayoutReference,
    reference_type: CheckoutPayoutReferenceType,
    // Merchant-supplied funding classification; required by the Checkout sender contract.
    // Absence is a missing-field error, never a substituted default.
    source_of_funds: CheckoutPayoutSourceOfFunds,
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutCorporateSender {
    company_name: Secret<String>,
    address: CheckoutPayoutAddress,
    reference: CheckoutPayoutReference,
    reference_type: CheckoutPayoutReferenceType,
    // Merchant-supplied funding classification; required by the Checkout sender contract.
    // Absence is a missing-field error, never a substituted default.
    source_of_funds: CheckoutPayoutSourceOfFunds,
}
#[derive(Debug, Serialize)]
struct CheckoutPayoutReference(String);

impl CheckoutPayoutReference {
    /// Sender reference is taken from `reference_id` only; there is no fallback.
    fn try_from_reference_id(
        reference_id: Option<&Secret<String>>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let reference = reference_id
            .ok_or_else(missing_field_err("vendor_account_details.reference_id"))?
            .peek();
        if reference.chars().count() > 15 {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "vendor_account_details.reference_id",
                context: domain_types::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Sender reference must not exceed 15 characters".to_owned(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }
        Ok(Self(reference.to_owned()))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckoutPayoutReferenceType {
    Other,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckoutPayoutSourceOfFunds {
    Credit,
    Debit,
    Prepaid,
    DepositAccount,
    MobileMoneyAccount,
    Cash,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckoutPayoutTokenType {
    Applepay,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckoutPayoutAccountHolderType {
    Individual,
}

impl CheckoutPayoutAddress {
    fn from_address_details(
        details: &AddressDetails,
        line1_field: &'static str,
        city_field: &'static str,
        country_field: &'static str,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        Ok(Self {
            address_line1: details
                .line1
                .clone()
                .ok_or_else(missing_field_err(line1_field))?,
            city: details
                .city
                .clone()
                .ok_or_else(missing_field_err(city_field))?,
            country: details
                .country
                .ok_or_else(missing_field_err(country_field))?,
            address_line2: details.line2.clone(),
            state: details
                .state
                .as_ref()
                .and_then(|state| state.code_or_name().cloned()),
            zip: details.zip.clone(),
        })
    }

    /// Destination account-holder address comes from billing only.
    fn try_from_billing_address(
        billing: &AddressDetails,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        Self::from_address_details(billing, "billing.line1", "billing.city", "billing.country")
    }

    /// Sender address comes from vendor-supplied details only; no billing fallback.
    fn try_from_sender_address(
        address: Option<&Address>,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let details = address
            .and_then(|address| address.address.as_ref())
            .ok_or_else(missing_field_err("vendor_account_details.address"))?;
        Self::from_address_details(
            details,
            "vendor_account_details.address.line1",
            "vendor_account_details.address.city",
            "vendor_account_details.address.country",
        )
    }
}

impl CheckoutPayoutAccountHolder {
    /// Destination account holder comes from billing only.
    fn try_from_billing(
        billing: &AddressDetails,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        Ok(Self {
            holder_type: CheckoutPayoutAccountHolderType::Individual,
            first_name: billing
                .first_name
                .clone()
                .ok_or_else(missing_field_err("billing.first_name"))?,
            last_name: billing
                .last_name
                .clone()
                .ok_or_else(missing_field_err("billing.last_name"))?,
            billing_address: CheckoutPayoutAddress::try_from_billing_address(billing)?,
        })
    }
}

impl From<PayoutSourceOfFunds> for CheckoutPayoutSourceOfFunds {
    fn from(source: PayoutSourceOfFunds) -> Self {
        match source {
            PayoutSourceOfFunds::Credit => Self::Credit,
            PayoutSourceOfFunds::Debit => Self::Debit,
            PayoutSourceOfFunds::Prepaid => Self::Prepaid,
            PayoutSourceOfFunds::DepositAccount => Self::DepositAccount,
            PayoutSourceOfFunds::MobileMoneyAccount => Self::MobileMoneyAccount,
            PayoutSourceOfFunds::Cash => Self::Cash,
        }
    }
}

impl CheckoutPayoutCorporateSender {
    /// Sender maps from vendor details only; no billing fallback.
    fn try_from_vendor_details(
        vendor: &PayoutVendorDetails,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        Ok(Self {
            company_name: vendor.vendor_name.clone().ok_or_else(missing_field_err(
                "vendor_account_details.vendor_details.vendor_name",
            ))?,
            address: CheckoutPayoutAddress::try_from_sender_address(vendor.address.as_ref())?,
            reference: CheckoutPayoutReference::try_from_reference_id(
                vendor.reference_id.as_ref(),
            )?,
            reference_type: CheckoutPayoutReferenceType::Other,
            source_of_funds: vendor
                .source_of_funds
                .ok_or_else(missing_field_err(
                    "vendor_account_details.vendor_details.source_of_funds",
                ))?
                .into(),
        })
    }
}

impl CheckoutPayoutIndividualSender {
    /// Sender maps from individual details only; no billing fallback.
    fn try_from_individual_details(
        individual: &PayoutIndividualDetails,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        Ok(Self {
            first_name: individual
                .first_name
                .clone()
                .ok_or_else(missing_field_err(
                    "vendor_account_details.individual_details.first_name",
                ))?,
            last_name: individual
                .last_name
                .clone()
                .ok_or_else(missing_field_err(
                    "vendor_account_details.individual_details.last_name",
                ))?,
            date_of_birth: individual.date_of_birth.clone(),
            address: CheckoutPayoutAddress::try_from_sender_address(individual.address.as_ref())?,
            reference: CheckoutPayoutReference::try_from_reference_id(
                individual.reference_id.as_ref(),
            )?,
            reference_type: CheckoutPayoutReferenceType::Other,
            source_of_funds: individual
                .source_of_funds
                .ok_or_else(missing_field_err(
                    "vendor_account_details.individual_details.source_of_funds",
                ))?
                .into(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<&PayoutTransferRequest<T>> for CheckoutPayoutSender {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(request: &PayoutTransferRequest<T>) -> Result<Self, Self::Error> {
        let party = request
            .vendor_account_details
            .as_ref()
            .ok_or_else(missing_field_err("vendor_account_details"))?;
        let vendor = party
            .vendor_details
            .as_ref()
            .ok_or_else(missing_field_err("vendor_account_details.vendor_details"))?;
        match vendor.vendor_type {
            Some(common_enums::BankHolderType::Business) => Ok(Self::Corporate(
                CheckoutPayoutCorporateSender::try_from_vendor_details(vendor)?,
            )),
            Some(common_enums::BankHolderType::Personal) => {
                let individual =
                    party
                        .individual_details
                        .as_ref()
                        .ok_or_else(missing_field_err(
                            "vendor_account_details.individual_details",
                        ))?;
                Ok(Self::Individual(
                    CheckoutPayoutIndividualSender::try_from_individual_details(individual)?,
                ))
            }
            _ => Err(missing_field_err(
                "vendor_account_details.vendor_details.vendor_type",
            )()),
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<T>,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for CheckoutPayoutRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CheckoutPayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<T>,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let creds = credentials(&data.connector_config)?;
        let billing = data.request.get_billing_address()?;
        let mut holder = CheckoutPayoutAccountHolder::try_from_billing(billing)?;
        if let Some(PayoutMethodData::Wallet(Wallet::ApplePayDecrypt(token))) =
            data.request.payout_method_data.as_ref()
        {
            if let Some(name) = token.card_holder_name.as_ref() {
                let mut parts = name.peek().split_whitespace();
                if let Some(first) = parts.next() {
                    let last = parts.collect::<Vec<_>>().join(" ");
                    if !last.is_empty() {
                        holder.first_name = Secret::new(first.to_owned());
                        holder.last_name = Secret::new(last);
                    }
                }
            }
        }
        let destination = match data.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::Wallet(Wallet::ApplePayDecrypt(token))) => {
                CheckoutPayoutMethod::NetworkToken(Box::new(CheckoutPayoutNetworkToken {
                    token: token.dpan.clone(),
                    expiry_month: Secret::new(
                        token
                            .expiry_month
                            .peek()
                            .parse()
                            .map_err(|_| missing_field_err("apple_pay.expiry_month")())?,
                    ),
                    expiry_year: Secret::new(
                        token
                            .expiry_year
                            .peek()
                            .parse()
                            .map_err(|_| missing_field_err("apple_pay.expiry_year")())?,
                    ),
                    token_type: CheckoutPayoutTokenType::Applepay,
                    cryptogram: None,
                    eci: None,
                    account_holder: holder,
                }))
            }
            Some(PayoutMethodData::Passthrough(token)) => {
                CheckoutPayoutMethod::PaymentInstrument(Box::new(CheckoutPaymentInstrument {
                    id: token.psp_token.clone(),
                    account_holder: holder,
                }))
            }
            _ => {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "payout_method_data",
                    context: Default::default(),
                }
                .into())
            }
        };
        Ok(Self {
            source: CheckoutPayoutSource::CurrencyAccount(CheckoutPayoutCurrencyAccount {
                id: creds.api_key.clone(),
            }),
            destination,
            amount: data.request.amount,
            currency: data.request.destination_currency,
            reference: data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            processing_channel_id: creds.processing_channel_id.clone(),
            instruction: CheckoutPayoutInstruction {
                funds_transfer_type: data
                    .request
                    .additional_payout_connector_details
                    .as_ref()
                    .and_then(|details| match details {
                        domain_types::payouts::payouts_types::AdditionalPayoutConnectorDetails::Checkout(
                            checkout,
                        ) => checkout.funds_transfer_type,
                    }),
            },
            sender: CheckoutPayoutSender::try_from(&data.request)?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct CheckoutPayoutGetRequest {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CheckoutPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    > for CheckoutPayoutGetRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _: CheckoutPayoutsRouterData<
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckoutPayoutResponse {
    id: String,
    status: CheckoutPayoutStatus,
    response_code: Option<String>,
    response_summary: Option<String>,
}
/// Checkout.com returns transfer statuses in PascalCase on the wire (e.g. `"Pending"`,
/// `"Paid"`); the lowercase forms are kept as deserialize aliases for safety.
#[derive(Debug, Serialize, Deserialize)]
enum CheckoutPayoutStatus {
    #[serde(alias = "accepted")]
    Accepted,
    #[serde(alias = "rejected")]
    Rejected,
    #[serde(alias = "pending")]
    Pending,
    #[serde(alias = "approved")]
    Approved,
    #[serde(alias = "declined")]
    Declined,
    /// Card payout delivered to the destination card (checkout.com payout lifecycle).
    #[serde(alias = "paid")]
    Paid,
    /// Bank payout returned by the receiving bank after funds were dispatched.
    #[serde(alias = "returned")]
    Returned,
}
impl From<&CheckoutPayoutStatus> for PayoutStatus {
    fn from(status: &CheckoutPayoutStatus) -> Self {
        match status {
            CheckoutPayoutStatus::Accepted => Self::Initiated,
            CheckoutPayoutStatus::Pending => Self::Pending,
            // `Approved` is terminal success for bank payouts, `Paid` for card payouts.
            CheckoutPayoutStatus::Approved | CheckoutPayoutStatus::Paid => Self::Success,
            CheckoutPayoutStatus::Returned => Self::Reversed,
            CheckoutPayoutStatus::Rejected | CheckoutPayoutStatus::Declined => Self::Failure,
        }
    }
}
impl CheckoutPayoutResponse {
    fn error(&self, code: u16) -> Option<ErrorResponse> {
        (PayoutStatus::from(&self.status) == PayoutStatus::Failure).then(|| ErrorResponse {
            status_code: code,
            code: self
                .response_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.into()),
            message: self
                .response_summary
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.into()),
            reason: self.response_summary.clone(),
            attempt_status: Some(FlowStatus::Payout(PayoutStatus::Failure)),
            connector_transaction_id: Some(self.id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CheckoutPayoutResponse, Self>>
    for RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest<T>,
        PayoutTransferResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<CheckoutPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item.response.error(item.http_code) {
            Some(error) => Err(error),
            None => Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: (&item.response.status).into(),
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
            }),
        };
        Ok(Self {
            response,
            ..item.router_data
        })
    }
}
impl TryFrom<ResponseRouterData<CheckoutPayoutResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<CheckoutPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item.response.error(item.http_code) {
            Some(error) => Err(error),
            None => Ok(PayoutGetResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: (&item.response.status).into(),
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
            }),
        };
        Ok(Self {
            response,
            ..item.router_data
        })
    }
}
