use std::fmt::Debug;

use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, Secret};
use serde::Serialize;

use super::{requests, responses, BarclaycardAmountConvertor, BarclaycardRouterData};
use crate::{types::ResponseRouterData, utils};

/// CAVV (Cardholder Authentication Verification Value) Algorithm
///
/// Barclaycard (Cybersource whitelabel) includes cavv_algorithm in ProcessingInformation
/// (unlike Cybersource which puts it only in 3DS-specific structures).
///
/// Value "2" = CVV with Authentication Transaction Number (ATN) - Standard for card payments
/// - This field is sent even for non-3DS payments (Barclaycard API requirement/recommendation)
/// - Barclaycard accepts and ignores it when no actual 3DS/CAVV data is present
/// - If 3DS were implemented in future, this would indicate the algorithm to use
const CAVV_ALGORITHM_ATN: &str = "2";

/// `processingInformation.commerceIndicator` — marks a standard e-commerce transaction.
/// Used for the initial customer-initiated flows (Authorize, SetupMandate) and for MITs
/// that reference a stored TMS payment instrument.
const COMMERCE_INDICATOR_INTERNET: &str = "internet";

/// `processingInformation.commerceIndicator` — marks a recurring merchant-initiated
/// transaction. Used when the MIT carries a raw card + network transaction id (NTI).
const COMMERCE_INDICATOR_RECURRING: &str = "recurring";

/// `paymentInformation.card.typeSelectionIndicator` — "1" tells Barclaycard that the
/// `type` field above refers to the primary card type (as opposed to a co-badged
/// secondary network). Barclaycard requires this for every card payment.
const TYPE_SELECTION_INDICATOR_PRIMARY: &str = "1";

/// `processingInformation.authorizationOptions.merchantInitiatedTransaction.reason` — "7"
/// indicates a merchant-initiated transaction using a network transaction id (the NTI
/// flow). Other reason codes exist for installment/recurring/resubmission MITs but are
/// not used here.
const MIT_REASON_NTI: &str = "7";

#[derive(Debug, Clone)]
pub struct BarclaycardAuthType {
    pub api_key: Secret<String>,
    pub merchant_account: Secret<String>,
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for BarclaycardAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Barclaycard {
                api_key,
                merchant_account,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_account: merchant_account.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

fn get_barclaycard_card_type(card_network: common_enums::CardNetwork) -> Option<&'static str> {
    match card_network {
        common_enums::CardNetwork::Visa => Some("001"),
        common_enums::CardNetwork::Mastercard => Some("002"),
        common_enums::CardNetwork::AmericanExpress => Some("003"),
        common_enums::CardNetwork::JCB => Some("007"),
        common_enums::CardNetwork::DinersClub => Some("005"),
        common_enums::CardNetwork::Discover => Some("004"),
        common_enums::CardNetwork::CartesBancaires => Some("006"),
        common_enums::CardNetwork::UnionPay => Some("062"),
        common_enums::CardNetwork::Maestro => Some("042"),
        common_enums::CardNetwork::Interac
        | common_enums::CardNetwork::RuPay
        | common_enums::CardNetwork::Star
        | common_enums::CardNetwork::Accel
        | common_enums::CardNetwork::Pulse
        | common_enums::CardNetwork::Nyce => None,
    }
}

/// Truncates a string to the specified maximum length
///
/// Barclaycard (Cybersource whitelabel) has a 20-character limit on the administrativeArea field.
/// Truncation prevents payment failures while maintaining address verification.
/// Most state names/codes are under 20 characters, so this rarely causes issues.
fn truncate_string(state: &Secret<String>, max_len: usize) -> Secret<String> {
    let exposed = state.clone().expose();
    let truncated = exposed.get(..max_len).unwrap_or(&exposed);
    Secret::new(truncated.to_string())
}

fn build_bill_to(
    billing: &domain_types::payment_address::Address,
    email: common_utils::pii::Email,
) -> Result<requests::BillTo, error_stack::Report<IntegrationError>> {
    let address = billing
        .address
        .as_ref()
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "billing.address",
            context: Default::default(),
        })?;

    let administrative_area = address
        .to_state_code_as_optional()
        .unwrap_or_else(|_| {
            address
                .state
                .as_ref()
                .map(|state| truncate_string(state, 20)) // NOTE: Cybersource connector throws error if billing state exceeds 20 characters. Since Barclaycard is Cybersource just whitelisting, we truncate if state exceeds 20 characters, so truncation is done to avoid payment failure
        })
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "billing_address.state",
            context: Default::default(),
        })?;

    Ok(requests::BillTo {
        first_name: address.get_first_name()?.clone(),
        last_name: address.get_last_name()?.clone(),
        address1: address.get_line1()?.clone(),
        locality: address.get_city()?.clone().expose(),
        administrative_area,
        postal_code: address.get_zip()?.clone(),
        country: *address.get_country()?,
        email,
    })
}

fn map_barclaycard_attempt_status(
    (status, auto_capture): (responses::BarclaycardPaymentStatus, bool),
) -> common_enums::AttemptStatus {
    match status {
        responses::BarclaycardPaymentStatus::Authorized
        | responses::BarclaycardPaymentStatus::AuthorizedPendingReview => {
            if auto_capture {
                // Because Barclaycard will return Payment Status as Authorized even in AutoCapture Payment
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Authorized
            }
        }
        responses::BarclaycardPaymentStatus::Pending => {
            if auto_capture {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::Pending
            }
        }
        responses::BarclaycardPaymentStatus::Succeeded
        | responses::BarclaycardPaymentStatus::Transmitted => common_enums::AttemptStatus::Charged,
        responses::BarclaycardPaymentStatus::Voided
        | responses::BarclaycardPaymentStatus::Reversed
        | responses::BarclaycardPaymentStatus::Cancelled => common_enums::AttemptStatus::Voided,
        responses::BarclaycardPaymentStatus::Failed
        | responses::BarclaycardPaymentStatus::Declined
        | responses::BarclaycardPaymentStatus::AuthorizedRiskDeclined
        | responses::BarclaycardPaymentStatus::InvalidRequest
        | responses::BarclaycardPaymentStatus::Rejected
        | responses::BarclaycardPaymentStatus::ServerError => common_enums::AttemptStatus::Failure,
        responses::BarclaycardPaymentStatus::PendingReview
        | responses::BarclaycardPaymentStatus::StatusNotReceived => {
            common_enums::AttemptStatus::Pending
        }
    }
}

fn map_barclaycard_refund_status(
    status: responses::BarclaycardRefundStatus,
    error_reason: Option<String>,
) -> common_enums::RefundStatus {
    match status {
        responses::BarclaycardRefundStatus::Succeeded
        | responses::BarclaycardRefundStatus::Transmitted => common_enums::RefundStatus::Success,
        responses::BarclaycardRefundStatus::Cancelled
        | responses::BarclaycardRefundStatus::Failed
        | responses::BarclaycardRefundStatus::Voided => common_enums::RefundStatus::Failure,
        responses::BarclaycardRefundStatus::Pending => common_enums::RefundStatus::Pending,
        responses::BarclaycardRefundStatus::TwoZeroOne => {
            if error_reason == Some("PROCESSOR_DECLINED".to_string()) {
                common_enums::RefundStatus::Failure
            } else {
                common_enums::RefundStatus::Pending
            }
        }
    }
}

/// Flatten Barclaycard's per-field error details into a single formatted string.
///
/// Barclaycard returns structured error details as a list of `{field, reason}` pairs.
/// This helper formats them into `"field1 : reason1, field2 : reason2"` for populating
/// the `reason` field of the UCS `ErrorResponse`.
fn format_error_details(details: Option<&Vec<responses::Details>>) -> Option<String> {
    details.map(|details| {
        details
            .iter()
            .map(|d| format!("{} : {}", d.field, d.reason))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

/// Extract and format the original authorized amount for Barclaycard MIT requests.
///
/// Barclaycard's `merchantInitiatedTransaction.originalAuthorizedAmount` references the
/// amount from the initial customer-initiated transaction that established the mandate.
/// It must be a decimal string in the major unit of the original currency (e.g. "10.00").
/// Both the ConnectorMandateId and NetworkMandateId branches of RepeatPayment need this.
fn get_repeat_payment_original_authorized_amount<T>(
    router_data: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
) -> Result<Option<String>, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    let original_pair = router_data
        .request
        .recurring_mandate_payment_data
        .as_ref()
        .and_then(|data| {
            data.original_payment_authorized_amount
                .as_ref()
                .map(|oa| (oa.amount, oa.currency))
        });

    match original_pair {
        Some((original_amount, original_currency)) => {
            Ok(Some(domain_types::utils::get_amount_as_string(
                &common_enums::CurrencyUnit::Base,
                original_amount,
                original_currency,
            )?))
        }
        None => Ok(None),
    }
}

fn get_error_reason(
    error_info: Option<String>,
    detailed_error_info: Option<String>,
    avs_error_info: Option<String>,
) -> Option<String> {
    match (error_info, detailed_error_info, avs_error_info) {
        (Some(message), Some(details), Some(avs_message)) => Some(format!(
            "{message}, detailed_error_information: {details}, avs_message: {avs_message}",
        )),
        (Some(message), Some(details), None) => {
            Some(format!("{message}, detailed_error_information: {details}"))
        }
        (Some(message), None, Some(avs_message)) => {
            Some(format!("{message}, avs_message: {avs_message}"))
        }
        (None, Some(details), Some(avs_message)) => {
            Some(format!("{details}, avs_message: {avs_message}"))
        }
        (Some(message), None, None) => Some(message),
        (None, Some(details), None) => Some(details),
        (None, None, Some(avs_message)) => Some(avs_message),
        (None, None, None) => None,
    }
}

/// Common response transformer for Authorize, Capture, and Void flows
///
/// This helper consolidates the shared logic for processing Barclaycard payment responses
/// across different payment flows. The only variation is the auto_capture flag which affects
/// status mapping.
fn transform_payment_response<F, Req>(
    item: ResponseRouterData<
        responses::BarclaycardPaymentsResponse,
        RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    >,
    auto_capture: bool,
) -> RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData> {
    match item.response {
        responses::BarclaycardPaymentsResponse::ClientReferenceInformation(info_response) => {
            let status = map_barclaycard_attempt_status((
                info_response
                    .status
                    .clone()
                    .unwrap_or(responses::BarclaycardPaymentStatus::StatusNotReceived),
                auto_capture,
            ));

            let response = if domain_types::utils::is_payment_failure(status) {
                Err(get_error_response(
                    &info_response.error_information,
                    &info_response.processor_information,
                    &info_response.risk_information,
                    Some(status),
                    item.http_code,
                    info_response.id.clone(),
                ))
            } else {
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(info_response.id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(
                        info_response
                            .client_reference_information
                            .code
                            .unwrap_or(info_response.id.clone()),
                    ),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                })
            };

            RouterDataV2 {
                response,
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }
        }
        responses::BarclaycardPaymentsResponse::ErrorInformation(error_response) => {
            let detailed_error_info =
                format_error_details(error_response.error_information.details.as_ref());

            let reason = get_error_reason(
                error_response.error_information.message.clone(),
                detailed_error_info,
                None,
            );

            RouterDataV2 {
                response: Err(ErrorResponse {
                    code: error_response
                        .error_information
                        .reason
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: error_response
                        .error_information
                        .reason
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason,
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(error_response.id),
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                }),
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }
        }
    }
}

fn get_error_response(
    error_data: &Option<responses::BarclaycardErrorInformation>,
    processor_information: &Option<responses::ClientProcessorInformation>,
    risk_information: &Option<responses::ClientRiskInformation>,
    attempt_status: Option<common_enums::AttemptStatus>,
    status_code: u16,
    transaction_id: String,
) -> ErrorResponse {
    let avs_message = risk_information
        .clone()
        .and_then(|client_risk_information| {
            client_risk_information.rules.map(|rules| {
                rules
                    .iter()
                    .map(|risk_info| {
                        risk_info.name.clone().map_or("".to_string(), |name| {
                            format!(" , {}", name.clone().expose())
                        })
                    })
                    .collect::<Vec<String>>()
                    .join("")
            })
        });

    let detailed_error_info = error_data
        .as_ref()
        .and_then(|error_info| format_error_details(error_info.details.as_ref()));

    let network_decline_code = processor_information
        .as_ref()
        .and_then(|info| info.response_code.clone());
    let network_advice_code = processor_information.as_ref().and_then(|info| {
        info.merchant_advice
            .as_ref()
            .and_then(|merchant_advice| merchant_advice.code_raw.clone())
    });

    let reason = get_error_reason(
        error_data
            .clone()
            .and_then(|error_details| error_details.message),
        detailed_error_info,
        avs_message,
    );
    let error_message = error_data
        .clone()
        .and_then(|error_details| error_details.reason);

    ErrorResponse {
        code: error_message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
        message: error_message
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason,
        status_code,
        attempt_status,
        connector_transaction_id: Some(transaction_id),
        network_advice_code,
        network_decline_code,
        network_error_message: None,
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = BarclaycardAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let ccard = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(card),
            _ => Err(IntegrationError::not_implemented(
                "Only card payments are supported".to_string(),
            )),
        }?;

        let card_network = ccard.card_network.clone();
        let card_type = card_network
            .and_then(get_barclaycard_card_type)
            .map(|s| s.to_string());

        let payment_information =
            requests::PaymentInformation::Cards(Box::new(requests::CardPaymentInformation {
                card: requests::Card {
                    number: ccard.card_number.clone(),
                    expiration_month: ccard.card_exp_month.clone(),
                    expiration_year: ccard.get_expiry_year_4_digit(),
                    security_code: ccard.card_cvc.clone(),
                    card_type,
                    type_selection_indicator: Some(TYPE_SELECTION_INDICATOR_PRIMARY.to_owned()),
                },
            }));

        let email = router_data
            .resource_common_data
            .get_billing_email()
            .or(router_data.request.get_email())?;

        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing",
                context: Default::default(),
            })?;

        let bill_to = build_bill_to(billing, email)?;

        let order_information = requests::OrderInformationWithBill {
            amount_details: requests::Amount {
                total_amount: amount,
                currency: router_data.request.currency,
            },
            bill_to: Some(bill_to),
        };

        let processing_information = requests::ProcessingInformation {
            commerce_indicator: COMMERCE_INDICATOR_INTERNET.to_string(),
            capture: Some(matches!(
                router_data.request.capture_method,
                Some(common_enums::CaptureMethod::Automatic) | None
            )),
            payment_solution: None, // Only set for wallet payments (GooglePay="012", ApplePay="001")
            cavv_algorithm: Some(CAVV_ALGORITHM_ATN.to_string()),
        };

        let client_reference_information = requests::ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        let merchant_defined_information = router_data
            .request
            .metadata
            .clone()
            .expose_option()
            .map(utils::convert_metadata_to_merchant_defined_info);

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
            merchant_defined_information,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::BarclaycardCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: BarclaycardRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &value.router_data;
        let amount = BarclaycardAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        let merchant_defined_information =
            router_data.request.metadata.clone().map(|metadata| {
                utils::convert_metadata_to_merchant_defined_info(metadata.expose())
            });

        Ok(Self {
            order_information: requests::OrderInformation {
                amount_details: requests::Amount {
                    total_amount: amount,
                    currency: router_data.request.currency,
                },
            },
            client_reference_information: requests::ClientReferenceInformation {
                code: Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
            },
            merchant_defined_information,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::BarclaycardVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: BarclaycardRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &value.router_data;
        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        let amount = BarclaycardAmountConvertor::convert(
            router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?,
            currency,
        )?;

        let merchant_defined_information =
            router_data.request.metadata.clone().map(|metadata| {
                utils::convert_metadata_to_merchant_defined_info(metadata.expose())
            });

        Ok(Self {
            client_reference_information: requests::ClientReferenceInformation {
                code: Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
            },
            reversal_information: requests::ReversalInformation {
                amount_details: requests::Amount {
                    total_amount: amount,
                    currency,
                },
                reason: router_data.request.cancellation_reason.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "cancellation_reason",
                        context: Default::default(),
                    },
                )?,
            },
            merchant_defined_information,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::BarclaycardRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = BarclaycardAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            order_information: requests::OrderInformation {
                amount_details: requests::Amount {
                    total_amount: amount,
                    currency: router_data.request.currency,
                },
            },
            client_reference_information: requests::ClientReferenceInformation {
                code: Some(router_data.request.refund_id.clone()),
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic) | None
        );
        Ok(transform_payment_response(item, auto_capture))
    }
}

impl TryFrom<ResponseRouterData<responses::BarclaycardPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(transform_payment_response(item, true))
    }
}

impl TryFrom<ResponseRouterData<responses::BarclaycardPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(transform_payment_response(item, false))
    }
}

impl TryFrom<ResponseRouterData<responses::BarclaycardTransactionResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardTransactionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.application_information.status {
            Some(app_status) => {
                let status = map_barclaycard_attempt_status((
                    app_status,
                    matches!(
                        item.router_data.request.capture_method,
                        Some(common_enums::CaptureMethod::Automatic) | None
                    ),
                ));

                if domain_types::utils::is_payment_failure(status) {
                    Ok(Self {
                        response: Err(get_error_response(
                            &item.response.error_information,
                            &item.response.processor_information,
                            &None,
                            Some(status),
                            item.http_code,
                            item.response.id.clone(),
                        )),
                        resource_common_data: PaymentFlowData {
                            status: common_enums::AttemptStatus::Failure,
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                } else {
                    Ok(Self {
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                item.response.id.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: item
                                .response
                                .client_reference_information
                                .and_then(|cref| cref.code)
                                .or(Some(item.response.id)),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        }),
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        ..item.router_data
                    })
                }
            }
            None => Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.id),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),
        }
    }
}

impl TryFrom<ResponseRouterData<responses::BarclaycardRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let error_reason = item
            .response
            .error_information
            .as_ref()
            .and_then(|error_info| error_info.reason.clone());

        let refund_status =
            map_barclaycard_refund_status(item.response.status.clone(), error_reason);

        let response = if utils::is_refund_failure(refund_status) {
            Err(get_error_response(
                &item.response.error_information,
                &None,
                &None,
                None,
                item.http_code,
                item.response.id.clone(),
            ))
        } else {
            Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            })
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<responses::BarclaycardRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item
            .response
            .application_information
            .and_then(|app_info| app_info.status)
        {
            Some(status) => {
                let error_reason = item
                    .response
                    .error_information
                    .as_ref()
                    .and_then(|error_info| error_info.reason.clone());

                let refund_status = map_barclaycard_refund_status(status.clone(), error_reason);

                if utils::is_refund_failure(refund_status) {
                    if status == responses::BarclaycardRefundStatus::Voided {
                        Err(get_error_response(
                            &Some(responses::BarclaycardErrorInformation {
                                message: Some("Refund voided".to_string()),
                                reason: Some("REFUND_VOIDED".to_string()),
                                details: None,
                            }),
                            &None,
                            &None,
                            None,
                            item.http_code,
                            item.response.id.clone(),
                        ))
                    } else {
                        Err(get_error_response(
                            &item.response.error_information,
                            &None,
                            &None,
                            None,
                            item.http_code,
                            item.response.id.clone(),
                        ))
                    }
                } else {
                    Ok(RefundsResponseData {
                        connector_refund_id: item.response.id,
                        refund_status,
                        status_code: item.http_code,
                    })
                }
            }
            None => Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: match item.router_data.response {
                    Ok(ref response) => response.refund_status,
                    Err(_) => common_enums::RefundStatus::Pending,
                },
                status_code: item.http_code,
            }),
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// --- SetupMandate (Zero-dollar auth for TMS token creation) transformers ---

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let email = router_data
            .resource_common_data
            .get_billing_email()
            .or(router_data.request.get_email())?;

        let billing = router_data
            .resource_common_data
            .address
            .get_payment_method_billing()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing",
                context: Default::default(),
            })?;

        let bill_to = build_bill_to(billing, email)?;

        let order_information = requests::OrderInformationWithBill {
            amount_details: requests::Amount {
                total_amount: StringMajorUnit::zero(),
                currency: router_data.request.currency,
            },
            bill_to: Some(bill_to),
        };

        let ccard = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(card),
            _ => Err(IntegrationError::not_implemented(
                "Only card payments are supported for mandate setup".to_string(),
            )),
        }?;

        let card_network = ccard.card_network.clone();
        let card_type = card_network
            .and_then(get_barclaycard_card_type)
            .map(|s| s.to_string());

        let payment_information =
            requests::PaymentInformation::Cards(Box::new(requests::CardPaymentInformation {
                card: requests::Card {
                    number: ccard.card_number.clone(),
                    expiration_month: ccard.card_exp_month.clone(),
                    expiration_year: ccard.get_expiry_year_4_digit(),
                    security_code: ccard.card_cvc.clone(),
                    card_type,
                    type_selection_indicator: Some(TYPE_SELECTION_INDICATOR_PRIMARY.to_owned()),
                },
            }));

        // Barclaycard (Smartpay) sandbox does not support Token Management Service (TMS).
        // Sending `actionList: [TOKEN_CREATE]` returns 502 SERVER_ERROR.
        // Instead, we perform a zero-dollar Customer-Initiated Transaction (CIT) to obtain a
        // network transaction id (NTI), which is then used as the mandate reference for
        // subsequent merchant-initiated transactions (MIT).
        let processing_information = requests::SetupMandateProcessingInformation {
            commerce_indicator: COMMERCE_INDICATOR_INTERNET.to_string(),
            capture: Some(false),
            action_list: None,
            action_token_types: None,
            authorization_options: Some(requests::SetupMandateAuthorizationOptions {
                initiator: Some(requests::SetupMandateInitiator {
                    initiator_type: Some(requests::BarclaycardPaymentInitiatorTypes::Customer),
                    credential_stored_on_file: Some(true),
                }),
            }),
        };

        let client_reference_information = requests::ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
        })
    }
}

// SetupMandate response transformer
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::BarclaycardPaymentsResponse::ClientReferenceInformation(info_response) => {
                // Only populate connector_mandate_id when TMS issued a payment instrument id.
                // When the sandbox cannot return a TMS token, the network_transaction_id is still
                // surfaced via network_txn_id below, letting the caller use the NetworkMandateId
                // MIT path — preserving the type distinction between a connector-generated
                // tokenized credential and a raw network transaction id.
                let mandate_reference = info_response
                    .token_information
                    .as_ref()
                    .and_then(|token_info| token_info.payment_instrument.as_ref())
                    .map(|payment_instrument| MandateReference {
                        connector_mandate_id: Some(payment_instrument.id.clone().expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                    });

                let mut status = map_barclaycard_attempt_status((
                    info_response
                        .status
                        .clone()
                        .unwrap_or(responses::BarclaycardPaymentStatus::StatusNotReceived),
                    false,
                ));

                // Drive zero-dollar mandate setups to a terminal attempt status. No money is
                // captured — the mandate flow has no separate capture step, so Authorized would
                // otherwise leave the attempt stuck in a non-terminal state. This matches the
                // hyperswitch Cybersource reference transformer for SetupMandate responses.
                if matches!(status, common_enums::AttemptStatus::Authorized) {
                    status = common_enums::AttemptStatus::Charged;
                }

                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(get_error_response(
                        &info_response.error_information,
                        &info_response.processor_information,
                        &info_response.risk_information,
                        Some(status),
                        item.http_code,
                        info_response.id.clone(),
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(info_response.id.clone()),
                        redirection_data: None,
                        mandate_reference: mandate_reference.map(Box::new),
                        connector_metadata: None,
                        network_txn_id: info_response.processor_information.as_ref().and_then(
                            |pi| {
                                pi.network_transaction_id
                                    .as_ref()
                                    .map(|ntid| ntid.clone().expose())
                            },
                        ),
                        connector_response_reference_id: Some(
                            info_response
                                .client_reference_information
                                .code
                                .unwrap_or(info_response.id.clone()),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    })
                };

                Ok(Self {
                    response,
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            responses::BarclaycardPaymentsResponse::ErrorInformation(error_response) => {
                let detailed_error_info =
                    format_error_details(error_response.error_information.details.as_ref());

                let reason = get_error_reason(
                    error_response.error_information.message.clone(),
                    detailed_error_info,
                    None,
                );

                Ok(Self {
                    response: Err(ErrorResponse {
                        code: error_response
                            .error_information
                            .reason
                            .clone()
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                        message: error_response
                            .error_information
                            .reason
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                        reason,
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(error_response.id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// --- RepeatPayment (MIT) transformers ---

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = BarclaycardAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let (commerce_indicator, authorization_options, payment_information) = match &router_data
            .request
            .mandate_reference
        {
            MandateReferenceId::ConnectorMandateId(_) => {
                let mandate_id = router_data.request.connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;
                let original_authorized_amount =
                    get_repeat_payment_original_authorized_amount(router_data)?;

                let payment_instrument = requests::PaymentInstrument {
                    id: mandate_id.into(),
                };

                let mandate_card = match router_data.request.payment_method_type {
                    Some(common_enums::PaymentMethodType::Card) => Some(requests::MandateCard {
                        type_selection_indicator: Some(
                            TYPE_SELECTION_INDICATOR_PRIMARY.to_owned(),
                        ),
                    }),
                    _ => None,
                };

                let pi = requests::RepeatPaymentInformation::MandatePayment(Box::new(
                    requests::MandatePaymentInformation {
                        payment_instrument,
                        card: mandate_card,
                    },
                ));

                // MIT using a stored TMS payment instrument. The paymentInstrument.id itself is
                // the reference to the stored credential, so previous_transaction_id/reason are
                // left unset (matches the hyperswitch Cybersource reference implementation).
                // initiator.type = Merchant + stored_credential_used = true tells Barclaycard
                // this is merchant-initiated against a stored credential.
                (
                    COMMERCE_INDICATOR_INTERNET.to_string(),
                    Some(requests::AuthorizationOptions {
                        initiator: Some(requests::PaymentInitiator {
                            initiator_type: Some(
                                requests::BarclaycardPaymentInitiatorTypes::Merchant,
                            ),
                            stored_credential_used: Some(true),
                        }),
                        merchant_initiated_transaction: Some(
                            requests::MerchantInitiatedTransaction {
                                reason: None,
                                original_authorized_amount,
                                previous_transaction_id: None,
                            },
                        ),
                    }),
                    pi,
                )
            }
            MandateReferenceId::NetworkMandateId(network_transaction_id) => {
                let original_authorized_amount =
                    get_repeat_payment_original_authorized_amount(router_data)?;

                let ccard = match &router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => Ok(card),
                    _ => Err(IntegrationError::MissingRequiredField {
                        field_name: "card details for network mandate MIT",
                        context: Default::default(),
                    }),
                }?;

                let card_type = ccard
                    .card_network
                    .clone()
                    .and_then(get_barclaycard_card_type)
                    .map(|s| s.to_string());

                let pi = requests::RepeatPaymentInformation::Cards(Box::new(
                    requests::CardWithNtiPaymentInformation {
                        card: requests::CardWithNti {
                            number: ccard.card_number.clone(),
                            expiration_month: ccard.card_exp_month.clone(),
                            expiration_year: ccard.card_exp_year.clone(),
                            security_code: None,
                            card_type,
                            type_selection_indicator: Some(
                                TYPE_SELECTION_INDICATOR_PRIMARY.to_owned(),
                            ),
                        },
                    },
                ));

                (
                    COMMERCE_INDICATOR_RECURRING.to_string(),
                    Some(requests::AuthorizationOptions {
                        initiator: Some(requests::PaymentInitiator {
                            initiator_type: Some(
                                requests::BarclaycardPaymentInitiatorTypes::Merchant,
                            ),
                            stored_credential_used: Some(true),
                        }),
                        merchant_initiated_transaction: Some(
                            requests::MerchantInitiatedTransaction {
                                reason: Some(MIT_REASON_NTI.to_string()),
                                original_authorized_amount,
                                previous_transaction_id: Some(Secret::new(
                                    network_transaction_id.clone(),
                                )),
                            },
                        ),
                    }),
                    pi,
                )
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::not_implemented(
                "Network token with NTI based MIT is not supported for Barclaycard".to_string(),
            ))?,
        };

        let processing_information = requests::RepeatPaymentProcessingInformation {
            commerce_indicator,
            capture: Some(matches!(
                router_data.request.capture_method,
                Some(common_enums::CaptureMethod::Automatic) | None
            )),
            authorization_options,
        };

        let bill_to = router_data
            .resource_common_data
            .get_optional_billing_email()
            .or(router_data.request.get_optional_email())
            .and_then(|email| {
                router_data
                    .resource_common_data
                    .get_optional_billing()
                    .and_then(|billing| build_bill_to(billing, email).ok())
            });

        let order_information = requests::OrderInformationWithBill {
            amount_details: requests::Amount {
                total_amount: amount,
                currency: router_data.request.currency,
            },
            bill_to,
        };

        let client_reference_information = requests::ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        let merchant_defined_information =
            router_data.request.metadata.clone().map(|metadata| {
                utils::convert_metadata_to_merchant_defined_info(metadata.expose())
            });

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
            merchant_defined_information,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::BarclaycardRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic) | None
        );
        match item.response {
            responses::BarclaycardPaymentsResponse::ClientReferenceInformation(info_response) => {
                let status = map_barclaycard_attempt_status((
                    info_response
                        .status
                        .clone()
                        .unwrap_or(responses::BarclaycardPaymentStatus::StatusNotReceived),
                    auto_capture,
                ));

                let response = if domain_types::utils::is_payment_failure(status) {
                    Err(get_error_response(
                        &info_response.error_information,
                        &info_response.processor_information,
                        &info_response.risk_information,
                        Some(status),
                        item.http_code,
                        info_response.id.clone(),
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(info_response.id.clone()),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(
                            info_response
                                .client_reference_information
                                .code
                                .unwrap_or(info_response.id.clone()),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    })
                };

                Ok(Self {
                    response,
                    resource_common_data: PaymentFlowData {
                        status,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            responses::BarclaycardPaymentsResponse::ErrorInformation(error_response) => {
                let detailed_error_info =
                    format_error_details(error_response.error_information.details.as_ref());

                let reason = get_error_reason(
                    error_response.error_information.message.clone(),
                    detailed_error_info,
                    None,
                );

                Ok(Self {
                    response: Err(ErrorResponse {
                        code: error_response
                            .error_information
                            .reason
                            .clone()
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
                        message: error_response
                            .error_information
                            .reason
                            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                        reason,
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: Some(error_response.id),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::Failure,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}
