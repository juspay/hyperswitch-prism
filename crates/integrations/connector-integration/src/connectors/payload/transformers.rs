use std::collections::HashMap;

use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    ext_traits::ValueExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference,
        PayloadClientAuthenticationResponse as PayloadClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{BankDebitData, PaymentMethodData, PaymentMethodDataTypes, WalletData},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse,
    },
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses};
use crate::connectors::payload::{PayloadAmountConvertor, PayloadRouterData};
use crate::types::ResponseRouterData;

pub use super::requests::{
    PayloadBankAccountRequestData, PayloadCaptureRequest, PayloadCardsRequestData,
    PayloadPaymentsRequest, PayloadRefundRequest, PayloadRepeatPaymentRequest, PayloadVoidRequest,
};
pub use super::responses::{
    PayloadAuthorizeResponse, PayloadCaptureResponse, PayloadErrorResponse, PayloadEventDetails,
    PayloadPSyncResponse, PayloadPaymentsResponse, PayloadRSyncResponse, PayloadRefundResponse,
    PayloadRepeatPaymentResponse, PayloadSetupMandateResponse, PayloadVoidResponse,
    PayloadWebhookEvent, PayloadWebhooksTrigger,
};

type Error = error_stack::Report<IntegrationError>;
type ResponseError = error_stack::Report<ConnectorError>;

// Constants
const PAYMENT_METHOD_TYPE_CARD: &str = "card";

// Helper function to check if capture method is manual
fn is_manual_capture(capture_method: Option<enums::CaptureMethod>) -> bool {
    matches!(capture_method, Some(enums::CaptureMethod::Manual))
}

// Auth Struct
#[derive(Debug, Clone, Deserialize)]
pub struct PayloadAuth {
    pub api_key: Secret<String>,
    pub processing_account_id: Option<Secret<String>>,
}

#[derive(Debug, Clone)]
pub struct PayloadAuthType {
    pub auths: HashMap<enums::Currency, PayloadAuth>,
}

impl TryFrom<(&ConnectorSpecificConfig, enums::Currency)> for PayloadAuth {
    type Error = Error;
    fn try_from(value: (&ConnectorSpecificConfig, enums::Currency)) -> Result<Self, Self::Error> {
        let (auth_type, currency) = value;
        match auth_type {
            ConnectorSpecificConfig::Payload { auth_key_map, .. } => auth_key_map
                .get(&currency)
                .ok_or(IntegrationError::CurrencyNotSupported {
                    message: currency.to_string(),
                    connector: "Payload",
                    context: Default::default(),
                })?
                .to_owned()
                .parse_value::<Self>("PayloadAuth")
                .change_context(IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for PayloadAuthType {
    type Error = Error;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Payload { auth_key_map, .. } => Ok(Self {
                auths: auth_key_map
                    .iter()
                    .map(|(currency, auth_value)| {
                        let auth = auth_value
                            .to_owned()
                            .parse_value::<PayloadAuth>("PayloadAuth")
                            .change_context(IntegrationError::FailedToObtainAuthType {
                                context: Default::default(),
                            })?;
                        Ok((*currency, auth))
                    })
                    .collect::<Result<_, Self::Error>>()?,
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// Helper function to build card request data
fn build_payload_cards_request_data<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
    connector_config: &ConnectorSpecificConfig,
    currency: enums::Currency,
    amount: FloatMajorUnit,
    resource_common_data: &PaymentFlowData,
    capture_method: Option<enums::CaptureMethod>,
    is_mandate: bool,
) -> Result<PayloadCardsRequestData<T>, Error> {
    if let PaymentMethodData::Card(req_card) = payment_method_data {
        let payload_auth = PayloadAuth::try_from((connector_config, currency))?;

        let card = requests::PayloadCard {
            number: req_card.card_number.clone(),
            expiry: req_card.get_card_expiry_month_year_2_digit_with_delimiter("/".to_string())?,
            cvc: req_card.card_cvc.clone(),
        };

        // Get billing address to access zip and state
        let billing_addr = resource_common_data.get_billing_address()?;

        let billing_address = requests::BillingAddress {
            city: resource_common_data.get_billing_city()?,
            country: resource_common_data.get_billing_country()?,
            postal_code: billing_addr.zip.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "billing.address.zip",
                    context: Default::default(),
                },
            )?,
            state_province: billing_addr.state.clone().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "billing.address.state",
                    context: Default::default(),
                },
            )?,
            street_address: resource_common_data.get_billing_line1()?,
        };

        // For manual capture, set status to "authorized"
        let status = if is_manual_capture(capture_method) {
            Some(responses::PayloadPaymentStatus::Authorized)
        } else {
            None
        };

        Ok(PayloadCardsRequestData {
            amount,
            card,
            transaction_types: requests::TransactionTypes::Payment,
            payment_method_type: PAYMENT_METHOD_TYPE_CARD.to_string(),
            status,
            billing_address,
            processing_id: payload_auth.processing_account_id,
            keep_active: is_mandate,
        })
    } else {
        Err(IntegrationError::NotSupported {
            message: "Payment method".to_string(),
            connector: "Payload",
            context: Default::default(),
        }
        .into())
    }
}

// Helper function to build bank account (ACH) request data
fn build_payload_bank_account_request_data(
    bank_debit_data: &BankDebitData,
    connector_config: &ConnectorSpecificConfig,
    currency: enums::Currency,
    amount: FloatMajorUnit,
    capture_method: Option<enums::CaptureMethod>,
    resource_common_data: &PaymentFlowData,
) -> Result<PayloadBankAccountRequestData, Error> {
    match bank_debit_data {
        BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            bank_account_holder_name,
            card_holder_name,
            bank_type,
            ..
        } => {
            let payload_auth = PayloadAuth::try_from((connector_config, currency))?;

            let account_holder = bank_account_holder_name
                .clone()
                .or_else(|| card_holder_name.clone())
                .or_else(|| resource_common_data.get_billing_full_name().ok())
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "bank_account_holder_name",
                    context: Default::default(),
                })?;

            let account_type = match bank_type {
                Some(enums::BankType::Savings) => requests::PayloadBankAccountType::Savings,
                Some(enums::BankType::Checking) | None => {
                    requests::PayloadBankAccountType::Checking
                }
                Some(enums::BankType::Transmission)
                | Some(enums::BankType::Current)
                | Some(enums::BankType::Bond)
                | Some(enums::BankType::SubscriptionShare) => {
                    Err(error_stack::report!(IntegrationError::NotSupported {
                        message: format!(
                            "Bank type {:?} is not supported for ACH bank debit",
                            bank_type
                        ),
                        connector: "Payload",
                        context: Default::default(),
                    }))?
                }
            };

            let bank_account = requests::PayloadBankAccount {
                account_number: account_number.clone(),
                routing_number: routing_number.clone(),
                account_type,
            };

            let status = if is_manual_capture(capture_method) {
                Some(responses::PayloadPaymentStatus::Authorized)
            } else {
                None
            };

            Ok(PayloadBankAccountRequestData {
                amount,
                bank_account,
                transaction_types: requests::TransactionTypes::Payment,
                payment_method_type: requests::PAYMENT_METHOD_TYPE_BANK_ACCOUNT.to_string(),
                account_holder,
                status,
                processing_id: payload_auth.processing_account_id,
                keep_active: false,
            })
        }
        BankDebitData::SepaBankDebit { .. }
        | BankDebitData::SepaGuaranteedBankDebit { .. }
        | BankDebitData::BecsBankDebit { .. }
        | BankDebitData::EftBankDebit { .. }
        | BankDebitData::BacsBankDebit { .. } => Err(IntegrationError::NotImplemented(
            domain_types::utils::get_unimplemented_payment_method_error_message("Payload"),
            Default::default(),
        )
        .into()),
    }
}

// TryFrom implementations for request bodies

// SetupMandate request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayloadCardsRequestData<T>
{
    type Error = Error;

    fn try_from(
        item: PayloadRouterData<
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

        match router_data.request.amount {
            Some(amount) if amount > 0 => Err(IntegrationError::FlowNotSupported {
                flow: "Setup mandate with non zero amount".to_string(),
                connector: "Payload".to_string(),
                context: Default::default(),
            }
            .into()),
            _ => {
                // For SetupMandate, is_mandate is always true
                build_payload_cards_request_data(
                    &router_data.request.payment_method_data,
                    &router_data.connector_config,
                    router_data.request.currency,
                    FloatMajorUnit::zero(),
                    &router_data.resource_common_data,
                    None, // No capture_method for SetupMandate
                    true,
                )
            }
        }
    }
}

// Authorize request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayloadPaymentsRequest<T>
{
    type Error = Error;

    fn try_from(
        item: PayloadRouterData<
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

        // Convert amount using PayloadAmountConvertor
        let amount = PayloadAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(_) => {
                let is_mandate = router_data.request.is_mandate_payment();

                let cards_data = build_payload_cards_request_data(
                    &router_data.request.payment_method_data,
                    &router_data.connector_config,
                    router_data.request.currency,
                    amount,
                    &router_data.resource_common_data,
                    router_data.request.capture_method,
                    is_mandate,
                )?;

                Ok(Self::PayloadCardsRequest(Box::new(cards_data)))
            }
            PaymentMethodData::BankDebit(bank_debit_data) => {
                let bank_account_data = build_payload_bank_account_request_data(
                    bank_debit_data,
                    &router_data.connector_config,
                    router_data.request.currency,
                    amount,
                    router_data.request.capture_method,
                    &router_data.resource_common_data,
                )?;

                Ok(Self::PayloadBankAccountRequest(Box::new(bank_account_data)))
            }
            // Payload connector supports GooglePay and ApplePay wallets, but not yet integrated
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                // ApplePayThirdPartySdk is handled directly in build_request_v2 (PUT /transactions/{token}).
                // get_request_body is never called for this payment method in the Authorize flow.
                WalletData::ApplePayThirdPartySdk(_) => Err(IntegrationError::not_implemented(
                    "ApplePayThirdPartySdk request body: handled via build_request_v2".to_string(),
                )
                .into()),
                WalletData::GooglePay(_) | WalletData::ApplePay(_) => {
                    Err(IntegrationError::not_implemented("Payment method".to_string()).into())
                }
                _ => Err(IntegrationError::NotSupported {
                    message: "Wallet".to_string(),
                    connector: "Payload",
                    context: Default::default(),
                }
                .into()),
            },
            // Payload.js Secure Inputs return a payment_method_id (pm_xxx) that is
            // sent server-side to /transactions as a top-level `payment_method_id`
            // form field — same wire shape as the repeat-payment path.
            // Docs: https://docs.payload.com/ui/payloadjs/secure-input/handle-results/
            PaymentMethodData::PaymentMethodToken(t) => {
                let token = t.token.clone();

                let status = if is_manual_capture(router_data.request.capture_method) {
                    Some(responses::PayloadPaymentStatus::Authorized)
                } else {
                    None
                };

                Ok(Self::PayloadCardTokenRequest(Box::new(
                    requests::PayloadCardTokenRequestData {
                        amount,
                        transaction_types: requests::TransactionTypes::Payment,
                        payment_method_id: token,
                        status,
                    },
                )))
            }
            _ => Err(IntegrationError::NotSupported {
                message: "Payment method".to_string(),
                connector: "Payload",
                context: Default::default(),
            }
            .into()),
        }
    }
}

// Capture request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PayloadCaptureRequest
{
    type Error = Error;

    fn try_from(
        _item: PayloadRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            status: responses::PayloadPaymentStatus::Processed,
        })
    }
}

// Void request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PayloadVoidRequest
{
    type Error = Error;

    fn try_from(
        _item: PayloadRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            status: responses::PayloadPaymentStatus::Voided,
        })
    }
}

// RepeatPayment request - for recurring/mandate payments
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<
                domain_types::connector_flow::RepeatPayment,
                PaymentFlowData,
                domain_types::connector_types::RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayloadRepeatPaymentRequest<T>
{
    type Error = Error;

    fn try_from(
        item: PayloadRouterData<
            RouterDataV2<
                domain_types::connector_flow::RepeatPayment,
                PaymentFlowData,
                domain_types::connector_types::RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Convert amount using PayloadAmountConvertor
        let amount = PayloadAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        // For manual capture, set status to "authorized"
        let status = if is_manual_capture(router_data.request.capture_method) {
            Some(responses::PayloadPaymentStatus::Authorized)
        } else {
            None
        };

        // RepeatPayment flow requires a mandate reference
        let mandate_id = match &router_data.request.mandate_reference {
            domain_types::connector_types::MandateReferenceId::ConnectorMandateId(
                connector_mandate_ref,
            ) => connector_mandate_ref.get_connector_mandate_id().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                },
            )?,
            _ => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id for RepeatPayment",
                    context: Default::default(),
                }
                .into())
            }
        };

        Ok(Self::PayloadMandateRequest(Box::new(
            requests::PayloadMandateRequestData {
                amount,
                transaction_types: requests::TransactionTypes::Payment,
                payment_method_id: Secret::new(mandate_id),
                status,
            },
        )))
    }
}

// Refund request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PayloadRefundRequest
{
    type Error = Error;

    fn try_from(
        item: PayloadRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert amount using PayloadAmountConvertor
        let amount = PayloadAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            transaction_type: requests::TransactionTypes::Refund,
            amount,
            ledger_assoc_transaction_id: connector_transaction_id,
        })
    }
}

// TryFrom implementations for response bodies

impl From<responses::PayloadPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: responses::PayloadPaymentStatus) -> Self {
        match item {
            responses::PayloadPaymentStatus::Authorized => Self::Authorized,
            responses::PayloadPaymentStatus::Processed => Self::Charged,
            responses::PayloadPaymentStatus::Processing => Self::Pending,
            responses::PayloadPaymentStatus::Rejected
            | responses::PayloadPaymentStatus::Declined => Self::Failure,
            responses::PayloadPaymentStatus::Voided => Self::Voided,
        }
    }
}

// Common function to handle PayloadPaymentsResponse
fn handle_payment_response<F, T>(
    response: PayloadPaymentsResponse,
    router_data: RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>,
    http_code: u16,
    is_mandate_payment: bool,
) -> RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData> {
    match response {
        PayloadPaymentsResponse::PayloadCardsResponse(card_response) => {
            let status = common_enums::AttemptStatus::from(card_response.status);

            let mandate_reference = if is_mandate_payment {
                let connector_payment_method_id = card_response
                    .connector_payment_method_id
                    .clone()
                    .expose_option();
                connector_payment_method_id.map(|id| MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                })
            } else {
                None
            };

            let connector_response = card_response
                .avs
                .map(|avs_response| {
                    let payment_checks = serde_json::json!({
                        "avs_result": avs_response
                    });
                    AdditionalPaymentMethodConnectorResponse::Card {
                        authentication_data: None,
                        payment_checks: Some(payment_checks),
                        card_network: None,
                        domestic_network: None,
                        auth_code: None,
                    }
                })
                .map(ConnectorResponseData::with_additional_payment_method_data);

            let response_result = if status == common_enums::AttemptStatus::Failure {
                Err(ErrorResponse {
                    attempt_status: None,
                    code: card_response
                        .status_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: card_response
                        .status_message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: card_response.status_message,
                    status_code: http_code,
                    connector_transaction_id: Some(card_response.transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            } else {
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(card_response.transaction_id),
                    redirection_data: None,
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: card_response.ref_number,
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                })
            };

            // Create a mutable copy to set the status
            let mut router_data_with_status = router_data;
            router_data_with_status
                .resource_common_data
                .set_status(status);

            RouterDataV2 {
                resource_common_data: PaymentFlowData {
                    connector_response,
                    ..router_data_with_status.resource_common_data
                },
                response: response_result,
                ..router_data_with_status
            }
        }
    }
}

// Authorize response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let is_mandate_payment = item.router_data.request.is_mandate_payment();
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            is_mandate_payment,
        ))
    }
}

// SetupMandate response
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // SetupMandate is always a mandate payment
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            true,
        ))
    }
}

// RepeatPayment response - for recurring/mandate payments
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::RepeatPayment,
        PaymentFlowData,
        domain_types::connector_types::RepeatPaymentData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // RepeatPayment should not return mandate_reference as the mandate already exists
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            false,
        ))
    }
}

// PSync response
impl TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        domain_types::connector_types::PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            false,
        ))
    }
}

// Capture response
impl TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            false,
        ))
    }
}

// Void response
impl TryFrom<ResponseRouterData<PayloadPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(handle_payment_response(
            item.response,
            item.router_data,
            item.http_code,
            false,
        ))
    }
}

// Refund status conversion
impl From<responses::RefundStatus> for enums::RefundStatus {
    fn from(item: responses::RefundStatus) -> Self {
        match item {
            responses::RefundStatus::Processed => Self::Success,
            responses::RefundStatus::Processing => Self::Pending,
            responses::RefundStatus::Declined | responses::RefundStatus::Rejected => Self::Failure,
        }
    }
}

// Refund response
impl TryFrom<ResponseRouterData<PayloadRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.to_string(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Webhook helper function to parse incoming webhook events
pub fn parse_webhook_event(
    body: &[u8],
) -> Result<PayloadWebhookEvent, error_stack::Report<IntegrationError>> {
    serde_json::from_slice::<PayloadWebhookEvent>(body).change_context(
        IntegrationError::not_implemented("webhook body decoding failed".to_string()),
    )
}

// RSync response
impl TryFrom<ResponseRouterData<PayloadRefundResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<PayloadRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.transaction_id.to_string(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Webhook event transformation
pub fn get_event_type_from_trigger(
    trigger: PayloadWebhooksTrigger,
) -> domain_types::connector_types::EventType {
    match trigger {
        // Payment Success Events
        PayloadWebhooksTrigger::Processed => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        PayloadWebhooksTrigger::Authorized => {
            domain_types::connector_types::EventType::PaymentIntentAuthorizationSuccess
        }
        // Payment Processing Events
        PayloadWebhooksTrigger::Payment | PayloadWebhooksTrigger::AutomaticPayment => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        // Payment Failure Events
        PayloadWebhooksTrigger::Decline
        | PayloadWebhooksTrigger::Reject
        | PayloadWebhooksTrigger::BankAccountReject => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        PayloadWebhooksTrigger::Void | PayloadWebhooksTrigger::Reversal => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        // Refund Events
        PayloadWebhooksTrigger::Refund => domain_types::connector_types::EventType::RefundSuccess,
        // Dispute Events
        PayloadWebhooksTrigger::Chargeback => {
            domain_types::connector_types::EventType::DisputeOpened
        }
        PayloadWebhooksTrigger::ChargebackReversal => {
            domain_types::connector_types::EventType::DisputeWon
        }
        // Other payment-related events - treat as generic payment processing
        PayloadWebhooksTrigger::PaymentActivationStatus
        | PayloadWebhooksTrigger::Credit
        | PayloadWebhooksTrigger::Deposit
        | PayloadWebhooksTrigger::PaymentLinkStatus
        | PayloadWebhooksTrigger::ProcessingStatus
        | PayloadWebhooksTrigger::TransactionOperation
        | PayloadWebhooksTrigger::TransactionOperationClear => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
    }
}

// ClientAuthenticationToken request — POST /access_tokens
#[derive(Debug, Serialize)]
pub struct PayloadClientAuthRequest {
    #[serde(rename = "type")]
    pub token_type: String,
    pub intent: PayloadClientAuthIntent,
}

#[derive(Debug, Serialize)]
pub struct PayloadClientAuthIntent {
    pub payment_form: PayloadClientAuthPaymentForm,
}

#[derive(Debug, Serialize)]
pub struct PayloadClientAuthPaymentForm {
    pub payment: PayloadClientAuthPayment,
}

#[derive(Debug, Serialize)]
pub struct PayloadClientAuthPayment {
    pub amount: FloatMajorUnit,
    pub description: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PayloadRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PayloadClientAuthRequest
{
    type Error = Error;
    fn try_from(
        item: PayloadRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        let amount = PayloadAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let description = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        Ok(Self {
            token_type: "client".to_string(),
            intent: PayloadClientAuthIntent {
                payment_form: PayloadClientAuthPaymentForm {
                    payment: PayloadClientAuthPayment {
                        amount,
                        description,
                    },
                },
            },
        })
    }
}

// ClientAuthenticationToken response
#[derive(Debug, Deserialize, Serialize)]
pub struct PayloadClientAuthResponse {
    pub id: Secret<String>,
}

impl TryFrom<ResponseRouterData<PayloadClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PayloadClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Payload(
                PayloadClientAuthenticationResponseDomain {
                    client_token: response.id,
                },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
