use super::PaysafePayoutsRouterData;
use crate::connectors::paysafe::transformers::PaysafeAuthType;
use crate::types::ResponseRouterData;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii::Email,
    MinorUnit,
};
use domain_types::{
    connector_flow::{PayoutCreateRecipient, PayoutTransfer},
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::PaymentMethodDataTypes,
    payouts::{
        payout_method_data::{GiftCardPayout, PayoutMethodData},
        payouts_types::{
            PayoutCreateRecipientRequest, PayoutCreateRecipientResponse, PayoutFlowData,
            PayoutTransferRequest, PayoutTransferResponse,
        },
    },
    router_data::{ErrorResponse, FlowStatus, PaysafeAccountKind},
    router_data_v2::RouterDataV2,
};
use error_stack::Report;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

fn unsupported_payout_method_error(flow: &str, supported: &str) -> Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: "Payout method is not supported".to_string(),
        connector: "Paysafe",
        context: IntegrationErrorContext {
            additional_context: Some(format!(
                "Paysafe {flow} - only the {supported} payout method is supported"
            )),
            suggested_action: Some(format!("Use a {supported} payout method")),
            doc_url: None,
        },
    }
    .into()
}

fn missing_field(field_name: &'static str, flow: &str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            additional_context: Some(format!("Paysafe {flow} - missing required field")),
            suggested_action: None,
            doc_url: None,
        },
    }
    .into()
}

fn invalid_date_of_birth(flow: &str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name: "request.vendor_account_details.individual_details.date_of_birth",
        context: IntegrationErrorContext {
            additional_context: Some(format!(
                "Paysafe {flow} - date of birth is invalid for the Paysafe API"
            )),
            suggested_action: None,
            doc_url: None,
        },
    }
    .into()
}

fn build_status_failure_response(
    status_code: u16,
    gateway_response: Option<&PaysafeGatewayResponse>,
    connector_transaction_id: String,
) -> ErrorResponse {
    let code = gateway_response
        .and_then(|gateway_response| gateway_response.code.clone())
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| NO_ERROR_CODE.to_owned());
    let message = gateway_response
        .and_then(|gateway_response| gateway_response.description.clone())
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_owned());
    ErrorResponse {
        status_code,
        code,
        message: message.clone(),
        reason: Some(message),
        attempt_status: Some(FlowStatus::Payout(common_enums::PayoutStatus::Failure)),
        connector_transaction_id: Some(connector_transaction_id),
        ..Default::default()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PaysafePaymentHandleMeta {
    payment_handle_token: Secret<String>,
}

fn to_payment_handle_meta(
    connector_meta: Option<serde_json::Value>,
) -> Result<PaysafePaymentHandleMeta, Report<IntegrationError>> {
    let json = connector_meta
        .ok_or_else(|| missing_field("payout_connector_metadata", "Payout Transfer"))?;
    serde_json::from_value(json).map_err(|_| {
        IntegrationError::InvalidDataFormat {
            field_name: "payout_connector_metadata",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Paysafe Payout Transfer - failed to parse payout_connector_metadata"
                        .to_string(),
                ),
                suggested_action: None,
                doc_url: None,
            },
        }
        .into()
    })
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleRequest {
    merchant_ref_num: String,
    transaction_type: PaysafeTransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_id: Option<Secret<String>>,
    payment_type: PaysafePaymentType,
    amount: MinorUnit,
    currency_code: common_enums::Currency,
    paysafecard: PaysafePaysafecardPayout,
    profile: PaysafeCustomerProfileDetails,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PaysafeTransactionType {
    StandaloneCredit,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
enum PaysafePaymentType {
    Paysafecard,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaysafecardPayout {
    consumer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    psc_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeCustomerProfileDetails {
    first_name: Secret<String>,
    last_name: Secret<String>,
    email: Email,
    date_of_birth: CustomerDateOfBirth,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CustomerDateOfBirth {
    day: u8,
    month: u8,
    year: u16,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest<T>,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    > for PaysafePaymentHandleRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutCreateRecipient,
                PayoutFlowData,
                PayoutCreateRecipientRequest<T>,
                PayoutCreateRecipientResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;

        let merchant_ref_num = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let account_id = PaysafeAuthType::try_from(&item.connector_config)?
            .account_id
            .and_then(|account_map| {
                account_map
                    .get_account_id(
                        PaysafeAccountKind::PaysafeGiftCard,
                        item.request.source_currency,
                    )
                    .ok()
            });

        let customer_details = item.request.customer.as_ref();

        let billing_address = item
            .request
            .address
            .as_ref()
            .and_then(|address| address.billing_address.as_ref());

        let email = billing_address
            .and_then(|billing| billing.email.clone())
            .or_else(|| customer_details.and_then(|customer| customer.email.clone()))
            .ok_or_else(|| {
                missing_field(
                    "request.customer.email or request.billing_address.email",
                    "Payout Create Recipient",
                )
            })?;

        let consumer_id = customer_details
            .and_then(|customer_details| customer_details.merchant_customer_id.clone())
            .ok_or_else(|| {
                missing_field(
                    "request.customer.merchant_customer_id",
                    "Payout Create Recipient",
                )
            })?;

        let gift_card = match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::GiftCard(GiftCardPayout::PaySafeCard(card))) => card,
            _ => {
                return Err(unsupported_payout_method_error(
                    "Payout Create Recipient",
                    "gift card (paysafecard account)",
                ))
            }
        };

        let psc_id = gift_card.paysafecard_account_id.clone();

        let paysafecard = PaysafePaysafecardPayout {
            consumer_id,
            psc_id,
        };

        let (dob_day, dob_month, dob_year) = match item.request.get_date_of_birth_parts()? {
            Some(date_of_birth_parts) => date_of_birth_parts,
            None => {
                return Err(missing_field(
                    "request.vendor_account_details.individual_details.date_of_birth",
                    "Payout Create Recipient",
                ))
            }
        };

        let day = dob_day
            .peek()
            .parse::<u8>()
            .map_err(|_| invalid_date_of_birth("Payout Create Recipient"))?;
        let month = dob_month
            .peek()
            .parse::<u8>()
            .map_err(|_| invalid_date_of_birth("Payout Create Recipient"))?;
        let year = dob_year
            .peek()
            .parse::<u16>()
            .map_err(|_| invalid_date_of_birth("Payout Create Recipient"))?;

        let profile = PaysafeCustomerProfileDetails {
            first_name: billing_address
                .and_then(|address| address.address.as_ref())
                .and_then(|address| address.first_name.clone())
                .ok_or_else(|| {
                    missing_field(
                        "request.billing_address.first_name",
                        "Payout Create Recipient",
                    )
                })?,
            last_name: billing_address
                .and_then(|address| address.address.as_ref())
                .and_then(|address| address.last_name.clone())
                .ok_or_else(|| {
                    missing_field(
                        "request.billing_address.last_name",
                        "Payout Create Recipient",
                    )
                })?,
            email,
            date_of_birth: CustomerDateOfBirth { day, month, year },
        };

        Ok(Self {
            merchant_ref_num,
            transaction_type: PaysafeTransactionType::StandaloneCredit,
            account_id,
            payment_type: PaysafePaymentType::Paysafecard,
            amount: item.request.amount,
            currency_code: item.request.source_currency,
            paysafecard,
            profile,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentHandleResponse {
    pub id: String,
    pub payment_handle_token: Secret<String>,
    pub status: PaysafeHandleStatus,
    pub gateway_reconciliation_id: Option<String>,
    pub gateway_response: Option<PaysafeGatewayResponse>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeHandleStatus {
    Initiated,
    Payable,
    Processing,
    Failed,
    Expired,
    Completed,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PaysafeGatewayResponse {
    pub code: Option<String>,
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaysafePaymentHandleResponse, Self>>
    for RouterDataV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest<T>,
        PayoutCreateRecipientResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafePaymentHandleResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_status = match item.response.status {
            PaysafeHandleStatus::Failed | PaysafeHandleStatus::Expired => {
                common_enums::PayoutStatus::Failure
            }
            PaysafeHandleStatus::Initiated
            | PaysafeHandleStatus::Payable
            | PaysafeHandleStatus::Processing
            | PaysafeHandleStatus::Completed => common_enums::PayoutStatus::RequiresFulfillment,
        };

        let response = if payout_status == common_enums::PayoutStatus::Failure {
            Err(build_status_failure_response(
                item.http_code,
                item.response.gateway_response.as_ref(),
                item.response.id.clone(),
            ))
        } else {
            let payout_connector_metadata = Some(Secret::new(serde_json::json!({
                "payment_handle_token": item.response.payment_handle_token,
            })));
            Ok(PayoutCreateRecipientResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status,
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
                payout_connector_metadata,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeStandaloneCreditRequest {
    merchant_ref_num: String,
    payment_handle_token: Secret<String>,
    amount: MinorUnit,
    currency_code: common_enums::Currency,
    description: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<T>,
                PayoutTransferResponse,
            >,
            T,
        >,
    > for PaysafeStandaloneCreditRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: PaysafePayoutsRouterData<
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<T>,
                PayoutTransferResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &item.router_data;
        match item.request.payout_method_data.as_ref() {
            Some(PayoutMethodData::GiftCard(_)) => {}
            _ => {
                return Err(unsupported_payout_method_error(
                    "Payout Transfer",
                    "gift card (paysafecard account)",
                ))
            }
        }

        let handle_meta = to_payment_handle_meta(
            item.request
                .payout_connector_metadata
                .clone()
                .map(|secret| secret.expose()),
        )?;

        let merchant_ref_num = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            merchant_ref_num,
            amount: item.request.amount,
            currency_code: item.request.destination_currency,
            payment_handle_token: handle_meta.payment_handle_token,
            description: item.resource_common_data.description.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeStandaloneCreditResponse {
    pub id: String,
    pub status: PaysafeTransactionRequestStatus,
    pub gateway_reconciliation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeTransactionRequestStatus {
    Received,
    Initiated,
    Pending,
    Failed,
    Cancelled,
    Expired,
    Completed,
}

impl From<PaysafeTransactionRequestStatus> for common_enums::PayoutStatus {
    fn from(item: PaysafeTransactionRequestStatus) -> Self {
        match item {
            PaysafeTransactionRequestStatus::Received
            | PaysafeTransactionRequestStatus::Initiated => Self::Initiated,
            PaysafeTransactionRequestStatus::Pending => Self::Pending,
            PaysafeTransactionRequestStatus::Completed => Self::Success,
            PaysafeTransactionRequestStatus::Failed => Self::Failure,
            PaysafeTransactionRequestStatus::Cancelled => Self::Cancelled,
            PaysafeTransactionRequestStatus::Expired => Self::Expired,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PaysafeStandaloneCreditResponse, Self>>
    for RouterDataV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest<T>,
        PayoutTransferResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeStandaloneCreditResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
                payout_status: common_enums::PayoutStatus::from(item.response.status),
                connector_payout_id: Some(item.response.id),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
