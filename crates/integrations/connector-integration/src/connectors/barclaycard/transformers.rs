use std::fmt::Debug;

use base64::Engine;
use common_utils::ext_traits::ValueExt;
use common_utils::types::StringMajorUnit;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, PeekInterface, Secret};
use serde::Serialize;

use super::{requests, responses, BarclaycardAmountConvertor, BarclaycardRouterData};
use crate::{
    types::ResponseRouterData,
    utils::{self, CardTypeCode},
};

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

/// `processingInformation.paymentSolution` code for ApplePay wallet payments.
/// Barclaycard mirrors Cybersource: ApplePay = "001", GooglePay = "012".
const APPLE_PAY_PAYMENT_SOLUTION: &str = "001";

/// `processingInformation.paymentSolution` code for GooglePay wallet payments.
/// Barclaycard mirrors Cybersource: ApplePay = "001", GooglePay = "012".
const GOOGLE_PAY_PAYMENT_SOLUTION: &str = "012";

/// `paymentInformation.fluidData.descriptor` for ApplePay encrypted tokens.
/// Base64 of `FID=COMMON.APPLE.INAPP.PAYMENT`, identical to the Cybersource value.
const FLUID_DATA_DESCRIPTOR: &str = "RklEPUNPTU1PTi5BUFBMRS5JTkFQUC5QQVlNRU5U";

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
    router_data: &RouterDataV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    >,
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
                    network_txn_link_id: None,
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
        attempt_status: attempt_status.map(FlowStatus::Payment),
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

        // Build the payment_information block and, for wallet payments, the
        // processingInformation.paymentSolution code. Card payments leave payment_solution
        // unset; ApplePay sets it to "001" (matching the Cybersource whitelabel mapping).
        //
        // Also compute the commerceIndicator (network-specific for encrypted ApplePay:
        // "spa" for Mastercard, "internet" otherwise; "internet" for Card, GooglePay and
        // decrypted ApplePay) and the consumerAuthenticationInformation (ApplePay sets
        // ucafCollectionIndicator="2" for Mastercard, None otherwise; Card/GooglePay None).
        let (
            payment_information,
            payment_solution,
            commerce_indicator,
            consumer_authentication_information,
        ): (
            requests::PaymentInformation<T>,
            Option<String>,
            String,
            Option<requests::ConsumerAuthenticationInformation>,
        ) = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => {
                // Derive the Cybersource card-type code from the card network, falling back to
                // the card-number issuer when the network is absent (cards arrive without an
                // enriched network in this flow). Mirrors the HS-Direct Barclaycard connector
                // so the request matches in shadow mode.
                let card_type = match ccard
                    .card_network
                    .clone()
                    .and_then(get_barclaycard_card_type)
                {
                    Some(code) => Some(code.to_string()),
                    None => domain_types::utils::get_card_issuer(ccard.card_number.peek())
                        .ok()
                        .and_then(|issuer| issuer.type_code())
                        .map(|s| s.to_string()),
                };

                (
                    requests::PaymentInformation::Cards(Box::new(
                        requests::CardPaymentInformation {
                            card: requests::Card {
                                number: ccard.card_number.clone(),
                                expiration_month: ccard.card_exp_month.clone(),
                                // HS-Direct passes the raw (2-digit) exp year through; match it
                                // for byte-parity instead of normalizing to 4-digit.
                                expiration_year: ccard.card_exp_year.clone(),
                                security_code: ccard.card_cvc.clone(),
                                card_type,
                                type_selection_indicator: Some(
                                    TYPE_SELECTION_INDICATOR_PRIMARY.to_owned(),
                                ),
                            },
                        },
                    )),
                    None,
                    COMMERCE_INDICATOR_INTERNET.to_string(),
                    None,
                )
            }
            PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => {
                // ApplePay sets consumerAuthenticationInformation.ucafCollectionIndicator="2"
                // for Mastercard (None otherwise) on both the decrypted and encrypted paths,
                // mirroring the Cybersource whitelabel reference.
                let is_mastercard =
                    apple_pay_data.payment_method.network.to_lowercase() == "mastercard";
                let consumer_authentication_information =
                    Some(requests::ConsumerAuthenticationInformation {
                        ucaf_collection_indicator: is_mastercard.then(|| "2".to_string()),
                        cavv: None,
                        ucaf_authentication_data: None,
                        xid: None,
                        directory_server_transaction_id: None,
                        specification_version: None,
                        pa_specification_version: None,
                        eci_raw: None,
                        pares_status: None,
                        acs_transaction_id: None,
                        cavv_algorithm: None,
                    });

                let (payment_information, commerce_indicator) = match apple_pay_data
                    .payment_data
                    .get_decrypted_apple_pay_payment_data_optional()
                {
                    // Decrypted token: send the PAN + cryptogram directly under tokenizedCard.
                    // commerceIndicator is always "internet" for the decrypted path.
                    Some(decrypt_data) => (
                        requests::PaymentInformation::ApplePay(Box::new(
                            requests::ApplePayPaymentInformation {
                                tokenized_card: requests::TokenizedCard {
                                    number: decrypt_data.clone().application_primary_account_number,
                                    cryptogram: Some(
                                        decrypt_data.clone().payment_data.online_payment_cryptogram,
                                    ),
                                    transaction_type: requests::TransactionType::InApp,
                                    expiration_year: decrypt_data.get_four_digit_expiry_year(),
                                    expiration_month: decrypt_data.get_expiry_month(),
                                },
                            },
                        )),
                        COMMERCE_INDICATOR_INTERNET.to_string(),
                    ),
                    // Encrypted token: forward the encrypted blob as fluidData and let
                    // Barclaycard decrypt it. commerceIndicator is network-specific:
                    // "spa" for Mastercard, "internet" otherwise.
                    None => {
                        let apple_pay_encrypted_data = apple_pay_data
                            .payment_data
                            .get_encrypted_apple_pay_payment_data_mandatory()
                            .change_context(IntegrationError::MissingRequiredField {
                                field_name: "Apple pay encrypted data",
                                context: Default::default(),
                            })?;
                        let commerce_indicator = if is_mastercard {
                            "spa".to_string()
                        } else {
                            COMMERCE_INDICATOR_INTERNET.to_string()
                        };
                        (
                            requests::PaymentInformation::ApplePayToken(Box::new(
                                requests::ApplePayTokenPaymentInformation {
                                    fluid_data: requests::FluidData {
                                        value: Secret::from(apple_pay_encrypted_data.clone()),
                                        descriptor: Some(FLUID_DATA_DESCRIPTOR.to_string()),
                                    },
                                    tokenized_card: requests::ApplePayTokenizedCard {
                                        transaction_type: requests::TransactionType::InApp,
                                    },
                                },
                            )),
                            commerce_indicator,
                        )
                    }
                };
                (
                    payment_information,
                    Some(APPLE_PAY_PAYMENT_SOLUTION.to_string()),
                    commerce_indicator,
                    consumer_authentication_information,
                )
            }
            PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                // Forward the encrypted Google Pay token as base64-encoded fluidData and
                // let Barclaycard decrypt it (mirrors the Cybersource whitelabel mapping).
                let google_pay_token = google_pay_data
                    .tokenization_data
                    .get_encrypted_google_pay_token()
                    .change_context(IntegrationError::MissingRequiredField {
                        field_name: "gpay wallet_token",
                        context: Default::default(),
                    })?;
                (
                    requests::PaymentInformation::GooglePay(Box::new(
                        requests::GooglePayTokenPaymentInformation {
                            fluid_data: requests::FluidData {
                                value: Secret::from(
                                    base64::engine::general_purpose::STANDARD
                                        .encode(google_pay_token),
                                ),
                                descriptor: None,
                            },
                        },
                    )),
                    Some(GOOGLE_PAY_PAYMENT_SOLUTION.to_string()),
                    COMMERCE_INDICATOR_INTERNET.to_string(),
                    None,
                )
            }
            _ => Err(IntegrationError::NotImplemented(
                "Selected payment method is not supported".to_string(),
                Default::default(),
            ))?,
        };

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
            commerce_indicator,
            capture: Some(matches!(
                router_data.request.capture_method,
                Some(common_enums::CaptureMethod::Automatic) | None
            )),
            payment_solution, // Only set for wallet payments (GooglePay="012", ApplePay="001")
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

        // Thread external 3DS authentication results into the Authorize request. When the
        // orchestrator supplies authentication_data (after the Authenticate/PostAuthenticate
        // steps), it takes precedence over the wallet-derived ucaf block. Non-3DS / wallet
        // flows keep their existing consumer_authentication_information unchanged.
        //
        // The cryptogram carrier is network-conditional (Mastercard -> ucafAuthenticationData,
        // Visa/others -> cavv), so pass the card network into the builder.
        let card_network_for_3ds = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => ccard.card_network.clone(),
            _ => None,
        };
        let consumer_authentication_information = router_data
            .request
            .authentication_data
            .clone()
            .map(|authn_data| {
                build_consumer_authentication_information(authn_data, card_network_for_3ds.clone())
            })
            .or(consumer_authentication_information);

        Ok(Self {
            processing_information,
            payment_information,
            order_information,
            client_reference_information,
            consumer_authentication_information,
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
                            network_txn_link_id: None,
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
                    network_txn_link_id: None,
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
            _ => Err(IntegrationError::NotImplemented(
                "Only card payments are supported for mandate setup".to_string(),
                Default::default(),
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
                        network_txn_link_id: None,
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
                        type_selection_indicator: Some(TYPE_SELECTION_INDICATOR_PRIMARY.to_owned()),
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
            MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotImplemented(
                "Network token with NTI based MIT is not supported for Barclaycard".to_string(),
                Default::default(),
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
                        network_txn_link_id: None,
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

// --- 3DS External Authentication (PreAuthenticate / Authenticate / PostAuthenticate) ---

/// Build the card `PaymentInformation` block shared by the three external-authentication
/// request builders. Mirrors the Card branch of the Authorize builder; only card payments
/// are supported for 3DS external authentication.
fn build_auth_card_payment_information<T>(
    ccard: &domain_types::payment_method_data::Card<T>,
) -> requests::PaymentInformation<T>
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
{
    // Derive the Cybersource card-type code from the card network, falling back to the
    // card-number issuer when the network is absent (the 3DS external-authentication cards
    // arrive without an enriched network). Mirrors the HS-Direct Barclaycard connector so the
    // PreAuthenticate / Authenticate requests match in shadow mode.
    let card_type = match ccard
        .card_network
        .clone()
        .and_then(get_barclaycard_card_type)
    {
        Some(code) => Some(code.to_string()),
        None => domain_types::utils::get_card_issuer(ccard.card_number.peek())
            .ok()
            .and_then(|issuer| issuer.type_code())
            .map(|s| s.to_string()),
    };

    requests::PaymentInformation::Cards(Box::new(requests::CardPaymentInformation {
        card: requests::Card {
            number: ccard.card_number.clone(),
            expiration_month: ccard.card_exp_month.clone(),
            // HS-Direct passes the raw (2-digit) exp year through; match it for byte-parity.
            expiration_year: ccard.card_exp_year.clone(),
            security_code: ccard.card_cvc.clone(),
            card_type,
            type_selection_indicator: Some(TYPE_SELECTION_INDICATOR_PRIMARY.to_owned()),
        },
    }))
}

/// Map external 3DS authentication results onto Barclaycard's `consumerAuthenticationInformation`.
///
/// Mirrors the HS-Direct Barclaycard connector field-for-field so the request is byte-identical:
/// - The cryptogram carrier is network-conditional: Mastercard puts it in `ucafAuthenticationData`
///   (with `ucafCollectionIndicator="2"`); Visa/others put it in `cavv` and leave
///   `ucafAuthenticationData` null. This needs the card network, which a bare `From` impl can't
///   see, so this is a function taking `card_network` explicitly.
/// - `specificationVersion` and `paSpecificationVersion` are both set from `message_version`.
/// - `paresStatus` is "Y" (authentication successful).
fn build_consumer_authentication_information(
    value: router_request_types::AuthenticationData,
    card_network: Option<common_enums::CardNetwork>,
) -> requests::ConsumerAuthenticationInformation {
    let router_request_types::AuthenticationData {
        eci: _,
        cavv,
        threeds_server_transaction_id: _,
        message_version,
        ds_trans_id,
        trans_status: _,
        acs_transaction_id: _,
        transaction_id,
        ucaf_collection_indicator: _,
        exemption_indicator: _,
        network_params: _,
    } = value;

    let (ucaf_authentication_data, cavv, ucaf_collection_indicator) =
        if card_network == Some(common_enums::CardNetwork::Mastercard) {
            (cavv.clone(), None, Some("2".to_string()))
        } else {
            (None, cavv, None)
        };

    requests::ConsumerAuthenticationInformation {
        cavv,
        ucaf_collection_indicator,
        ucaf_authentication_data,
        xid: transaction_id,
        directory_server_transaction_id: ds_trans_id.map(Secret::new),
        specification_version: message_version.clone(),
        pa_specification_version: message_version,
        eci_raw: None,
        pares_status: Some(requests::BarclaycardParesStatus::AuthenticationSuccessful),
        acs_transaction_id: None,
        cavv_algorithm: None,
    }
}

/// Redirect-payload returned by the device-data-collection step, parsed in PostAuthenticate.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BarclaycardRedirectionAuthResponse {
    pub transaction_id: String,
}

/// Build the `AuthenticationData` surfaced to the orchestrator from a validate response.
///
/// Mirrors Cybersource: `cavv` is populated from UCAF data when present (Mastercard) else from
/// the `cavv` field, `eci` from `indicator`.
fn get_authentication_data_for_validation_response(
    response: responses::BarclaycardConsumerAuthInformationEnrollmentResponse,
) -> router_request_types::AuthenticationData {
    let trans_status = response
        .validate_response
        .pares_status
        .map(common_enums::TransactionStatus::from);
    let cavv = response
        .validate_response
        .ucaf_authentication_data
        .or(response.validate_response.cavv);
    let eci = response.validate_response.indicator;
    let ucaf_collection_indicator = response.validate_response.ucaf_collection_indicator.clone();
    let ds_trans_id = response
        .validate_response
        .directory_server_transaction_id
        .map(|id| id.expose());
    router_request_types::AuthenticationData {
        ucaf_collection_indicator,
        eci,
        cavv,
        threeds_server_transaction_id: response.validate_response.three_d_s_server_transaction_id,
        message_version: response.validate_response.specification_version,
        trans_status,
        ds_trans_id,
        acs_transaction_id: response.validate_response.acs_transaction_id,
        transaction_id: response.validate_response.xid,
        exemption_indicator: None,
        network_params: None,
    }
}

/// Frictionless `AuthenticationData` for the Authenticate response.
///
/// Mirrors Cybersource: `eci` comes from `indicator`; `threeds_server_transaction_id`,
/// `acs_transaction_id` and `trans_status` are intentionally left unset.
fn get_authentication_data_for_authenticate_response(
    validate_response: &responses::BarclaycardConsumerAuthValidateResponse,
) -> router_request_types::AuthenticationData {
    router_request_types::AuthenticationData {
        eci: validate_response.indicator.clone(),
        cavv: validate_response.cavv.clone(),
        threeds_server_transaction_id: None,
        message_version: validate_response.specification_version.clone(),
        ds_trans_id: validate_response
            .directory_server_transaction_id
            .as_ref()
            .map(|id| id.clone().expose()),
        acs_transaction_id: None,
        trans_status: None,
        transaction_id: validate_response.xid.clone(),
        ucaf_collection_indicator: validate_response.ucaf_collection_indicator.clone(),
        exemption_indicator: None,
        network_params: None,
    }
}

/// `three_ds_data` JSON surfaced in `connector_feature_data` for the Authenticate response.
fn get_three_ds_data_for_authenticate_response(
    validate_response: &responses::BarclaycardConsumerAuthValidateResponse,
) -> serde_json::Value {
    serde_json::json!({
        "ucafCollectionIndicator": validate_response.ucaf_collection_indicator,
        "cavv": validate_response.cavv,
        "ucafAuthenticationData": validate_response.ucaf_authentication_data,
        "xid": validate_response.xid,
        "specificationVersion": validate_response.specification_version,
        "directoryServerTransactionId": validate_response.directory_server_transaction_id,
        "indicator": validate_response.indicator,
    })
}

fn build_auth_error_response(
    error_response: responses::BarclaycardErrorInformationResponse,
    http_code: u16,
) -> ErrorResponse {
    let detailed_error_info =
        format_error_details(error_response.error_information.details.as_ref());
    let reason = get_error_reason(
        error_response.error_information.message.clone(),
        detailed_error_info,
        None,
    );
    let error_message = error_response.error_information.reason;
    ErrorResponse {
        code: error_message
            .clone()
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
        message: error_message
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
        reason,
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: Some(error_response.id),
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    }
}

// --- PreAuthenticate (risk/v1/authentication-setups) ---

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardAuthSetupRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let payment_method_data = router_data.request.payment_method_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: Default::default(),
            },
        )?;

        match payment_method_data {
            PaymentMethodData::Card(ccard) => Ok(Self {
                payment_information: build_auth_card_payment_information(ccard),
                client_reference_information: requests::ClientReferenceInformation {
                    code: Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    ),
                },
            }),
            _ => Err(IntegrationError::NotImplemented(
                "Selected payment method is not supported for 3DS authentication".to_string(),
                Default::default(),
            ))?,
        }
    }
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardAuthSetupResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::BarclaycardAuthSetupResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::BarclaycardAuthSetupResponse::ClientAuthSetupInfo(info_response) => {
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                        resource_id: None,
                        redirection_data: Some(Box::new(RedirectForm::CybersourceAuthSetup {
                            access_token: info_response
                                .consumer_authentication_information
                                .access_token
                                .expose(),
                            ddc_url: info_response
                                .consumer_authentication_information
                                .device_data_collection_url,
                            reference_id: info_response
                                .consumer_authentication_information
                                .reference_id,
                        })),
                        connector_response_reference_id: Some(
                            info_response
                                .client_reference_information
                                .code
                                .unwrap_or(info_response.id.clone()),
                        ),
                        status_code: item.http_code,
                        authentication_data: None,
                    }),
                    ..item.router_data
                })
            }
            responses::BarclaycardAuthSetupResponse::ErrorInformation(error_response) => Ok(Self {
                response: Err(build_auth_error_response(*error_response, item.http_code)),
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            }),
        }
    }
}

// --- Authenticate (risk/v1/authentications) ---

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardAuthEnrollmentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let client_reference_information = requests::ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        let payment_method_data = router_data.request.payment_method_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: Default::default(),
            },
        )?;
        let payment_information = match payment_method_data {
            PaymentMethodData::Card(ccard) => build_auth_card_payment_information(ccard),
            _ => Err(IntegrationError::NotImplemented(
                "Selected payment method is not supported for 3DS authentication".to_string(),
                Default::default(),
            ))?,
        };

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        let total_amount =
            BarclaycardAmountConvertor::convert(router_data.request.amount, currency)?;

        let email = router_data
            .resource_common_data
            .get_billing_email()
            .or(router_data.request.email.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: Default::default(),
                },
            ))?;

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
                total_amount,
                currency,
            },
            bill_to: Some(bill_to),
        };

        let redirect_response = router_data.request.redirect_response.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "redirect_response",
                context: Default::default(),
            },
        )?;
        let param = redirect_response
            .params
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "request.redirect_response.params",
                context: Default::default(),
            })?;
        let reference_id = param
            .peek()
            .split('=')
            .next_back()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "request.redirect_response.params.reference_id",
                context: Default::default(),
            })?
            .to_string();

        let return_url = router_data
            .request
            .continue_redirection_url
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: Default::default(),
            })?
            .to_string();

        Ok(Self {
            payment_information,
            client_reference_information,
            consumer_authentication_information:
                requests::BarclaycardConsumerAuthInformationRequest {
                    return_url,
                    reference_id,
                },
            order_information,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::BarclaycardAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::BarclaycardAuthenticateResponse::ClientAuthCheckInfo(info_response) => {
                let status = common_enums::AttemptStatus::from(info_response.status);
                if domain_types::utils::is_payment_failure(status) {
                    let response = Err(get_error_response(
                        &info_response.error_information,
                        &None,
                        &None,
                        Some(status),
                        item.http_code,
                        info_response.id.clone(),
                    ));
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response,
                        ..item.router_data
                    })
                } else {
                    let connector_response_reference_id = Some(
                        info_response
                            .client_reference_information
                            .code
                            .unwrap_or(info_response.id.clone()),
                    );

                    // Challenge flow: access_token + step_up_url both present => redirect the
                    // cardholder to the ACS. Otherwise the authentication is frictionless.
                    let redirection_data = match (
                        info_response
                            .consumer_authentication_information
                            .access_token
                            .clone(),
                        info_response
                            .consumer_authentication_information
                            .step_up_url
                            .clone(),
                    ) {
                        (Some(token), Some(step_up_url)) => {
                            Some(RedirectForm::CybersourceConsumerAuth {
                                access_token: token.expose(),
                                step_up_url,
                            })
                        }
                        _ => None,
                    };

                    let validate_response = &info_response
                        .consumer_authentication_information
                        .validate_response;
                    let connector_feature_data = Some(serde_json::json!({
                        "three_ds_data":
                            get_three_ds_data_for_authenticate_response(validate_response)
                    }));
                    let authentication_data = Some(
                        get_authentication_data_for_authenticate_response(validate_response),
                    );

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::AuthenticateResponse {
                            resource_id: None,
                            redirection_data: redirection_data.map(Box::new),
                            connector_response_reference_id,
                            authentication_data,
                            connector_feature_data,
                            status_code: item.http_code,
                        }),
                        ..item.router_data
                    })
                }
            }
            responses::BarclaycardAuthenticateResponse::ErrorInformation(error_response) => {
                Ok(Self {
                    response: Err(build_auth_error_response(*error_response, item.http_code)),
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}

// --- PostAuthenticate (risk/v1/authentication-results) ---

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BarclaycardRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::BarclaycardAuthValidateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: BarclaycardRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let client_reference_information = requests::ClientReferenceInformation {
            code: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        };

        let payment_method_data = router_data.request.payment_method_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: Default::default(),
            },
        )?;
        let payment_information = match payment_method_data {
            PaymentMethodData::Card(ccard) => build_auth_card_payment_information(ccard),
            _ => Err(IntegrationError::NotImplemented(
                "Selected payment method is not supported for 3DS authentication".to_string(),
                Default::default(),
            ))?,
        };

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        let total_amount =
            BarclaycardAmountConvertor::convert(router_data.request.amount, currency)?;
        let order_information = requests::OrderInformation {
            amount_details: requests::Amount {
                total_amount,
                currency,
            },
        };

        let redirect_response = router_data.request.redirect_response.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "redirect_response",
                context: Default::default(),
            },
        )?;
        let redirection_response: BarclaycardRedirectionAuthResponse = redirect_response
            .payload
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "request.redirect_response.payload",
                context: Default::default(),
            })?
            .expose()
            .parse_value("BarclaycardRedirectionAuthResponse")
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "BarclaycardRedirectionAuthResponse",
                context: Default::default(),
            })?;

        Ok(Self {
            payment_information,
            client_reference_information,
            consumer_authentication_information:
                requests::BarclaycardConsumerAuthInformationValidateRequest {
                    authentication_transaction_id: redirection_response.transaction_id,
                },
            order_information,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::BarclaycardPostAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<responses::BarclaycardPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::BarclaycardAuthenticateResponse::ClientAuthCheckInfo(info_response) => {
                let status = common_enums::AttemptStatus::from(info_response.status);
                if domain_types::utils::is_payment_failure(status) {
                    let response = Err(get_error_response(
                        &info_response.error_information,
                        &None,
                        &None,
                        Some(status),
                        item.http_code,
                        info_response.id.clone(),
                    ));
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..item.router_data.resource_common_data
                        },
                        response,
                        ..item.router_data
                    })
                } else {
                    let connector_response_reference_id = Some(
                        info_response
                            .client_reference_information
                            .code
                            .unwrap_or(info_response.id.clone()),
                    );

                    let validate_response = &info_response
                        .consumer_authentication_information
                        .validate_response;
                    let three_ds_data = serde_json::to_value(validate_response).change_context(
                        ConnectorError::ResponseDeserializationFailed {
                            context: Default::default(),
                        },
                    )?;
                    let connector_feature_data = Some(Secret::new(serde_json::json!({
                        "three_ds_data": three_ds_data
                    })));

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            connector_feature_data,
                            ..item.router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                            authentication_data: Some(
                                get_authentication_data_for_validation_response(
                                    info_response.consumer_authentication_information,
                                ),
                            ),
                            connector_response_reference_id,
                            status_code: item.http_code,
                        }),
                        ..item.router_data
                    })
                }
            }
            responses::BarclaycardAuthenticateResponse::ErrorInformation(error_response) => {
                Ok(Self {
                    response: Err(build_auth_error_response(*error_response, item.http_code)),
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationFailed,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
        }
    }
}
