use std::fmt::Debug;

use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::MerchanteRouterData;
use crate::types::ResponseRouterData;

/// Distinct type alias so the flow-implementation macro generates a
/// `MerchanteRefundSyncRequestTemplating` marker separate from PSync's
/// `MerchanteSyncRequestTemplating` — the shape is identical, only the
/// generated marker name has to differ.
pub type MerchanteRefundSyncRequest = MerchanteSyncRequest;

// ============================================================================
// CONSTANTS — Merchante `transaction_type` codes and gateway sentinels
// ============================================================================
const TXN_TYPE_SALE: &str = "D";
const TXN_TYPE_PREAUTH: &str = "P";
const TXN_TYPE_SETTLE: &str = "S";
const TXN_TYPE_VOID: &str = "V";
const TXN_TYPE_REFUND: &str = "U";
const TXN_TYPE_INQUIRY: &str = "I";
const TXN_TYPE_VERIFY: &str = "A";

const ECOM_INDICATOR_DEFAULT: &str = "7";
const ECOM_INDICATOR_RECURRING_MOTO: &str = "2";
const ACCOUNT_DATA_SOURCE_KEYED: &str = "@";
const ACCOUNT_DATA_SOURCE_COF: &str = "Y";
const CIT_MIT_UNSCHEDULED: &str = "M101";

// Approved / partial-approval outcomes; anything else is a decline or gateway error.
const MERCHANTE_APPROVED: &str = "000";
const MERCHANTE_PARTIAL_APPROVED: &str = "010";

// The retry_id field is capped at 16 chars by the gateway. connector_request_reference_id
// can be longer, so we truncate defensively.
const RETRY_ID_MAX_LEN: usize = 16;

// ============================================================================
// AUTHENTICATION
// ============================================================================

pub struct MerchanteAuthType {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for MerchanteAuthType {
    type Error = Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Merchante {
                profile_id,
                profile_key,
                ..
            } => Ok(Self {
                profile_id: profile_id.to_owned(),
                profile_key: profile_key.to_owned(),
            }),
            _ => Err(Report::new(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Configure this merchant account's Merchante connector with a \
                         profile_id and profile_key (ConnectorSpecificConfig::Merchante)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "The connector_config passed to MerchanteAuthType::try_from was not \
                         the ConnectorSpecificConfig::Merchante variant."
                            .to_string(),
                    ),
                },
            })),
        }
    }
}

// ============================================================================
// SHARED HELPERS
// ============================================================================

fn truncate_retry_id(reference: &str) -> String {
    if reference.len() <= RETRY_ID_MAX_LEN {
        reference.to_string()
    } else {
        reference[..RETRY_ID_MAX_LEN].to_string()
    }
}

/// Merchante's Sale (`D`) auto-captures; Preauth (`P`) requires a later Settle.
fn sale_txn_type(is_auto_capture: bool) -> &'static str {
    if is_auto_capture {
        TXN_TYPE_SALE
    } else {
        TXN_TYPE_PREAUTH
    }
}

// ============================================================================
// COMMON RESPONSE + ERROR TYPES (shared by every flow)
// ============================================================================

/// Merchante returns the same flat urlencoded shape from every endpoint.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MerchantePaymentResponse {
    pub transaction_id: String,
    pub error_code: String,
    #[serde(default)]
    pub auth_response_text: Option<String>,
    #[serde(default)]
    pub auth_code: Option<String>,
    #[serde(default)]
    pub avs_result: Option<String>,
    #[serde(default)]
    pub cvv2_result: Option<String>,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub payment_account_reference: Option<String>,
    #[serde(default)]
    pub retry_count: Option<String>,
    #[serde(default)]
    pub partial_auth: Option<String>,
}

impl MerchantePaymentResponse {
    fn is_approved(&self) -> bool {
        self.error_code == MERCHANTE_APPROVED || self.error_code == MERCHANTE_PARTIAL_APPROVED
    }

    fn to_error_response(&self, status_code: u16) -> ErrorResponse {
        // Gateway may return a decline body where the fields are blank; surface
        // NO_ERROR_CODE / NO_ERROR_MESSAGE rather than fabricating a message.
        let code = if self.error_code.is_empty() {
            NO_ERROR_CODE.to_string()
        } else {
            self.error_code.clone()
        };
        let message = self
            .auth_response_text
            .clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
        ErrorResponse {
            status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: Some(self.transaction_id.clone()),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }
    }
}

// ============================================================================
// STATUS MAPPINGS
// ============================================================================

/// Merchante's outcome is expressed via `error_code`: `000` (Approved) or
/// `010` (Partial Approval) → approved; anything else → declined. We normalise
/// that into an enum first, then convert into Hyperswitch's status types via
/// `From`. Auth-vs-capture context (needed for AttemptStatus) is supplied via
/// a tuple conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MerchantePaymentStatus {
    Approved,
    Declined,
}

impl From<&MerchantePaymentResponse> for MerchantePaymentStatus {
    fn from(response: &MerchantePaymentResponse) -> Self {
        if response.is_approved() {
            Self::Approved
        } else {
            Self::Declined
        }
    }
}

impl MerchantePaymentStatus {
    /// Combine the parsed outcome with the caller's capture intent to produce
    /// a Hyperswitch `AttemptStatus`. Kept as an inherent method rather than a
    /// `From<(Self, bool)>` impl because the orphan rule forbids implementing
    /// `From<(LocalType, bool)>` for `AttemptStatus` (bool + AttemptStatus are
    /// both foreign; the tuple is foreign too).
    fn to_attempt_status(self, is_auto_capture: bool) -> AttemptStatus {
        match self {
            Self::Approved if is_auto_capture => AttemptStatus::Charged,
            Self::Approved => AttemptStatus::Authorized,
            Self::Declined => AttemptStatus::Failure,
        }
    }
}

impl From<MerchantePaymentStatus> for RefundStatus {
    fn from(status: MerchantePaymentStatus) -> Self {
        match status {
            MerchantePaymentStatus::Approved => Self::Success,
            MerchantePaymentStatus::Declined => Self::Failure,
        }
    }
}

// ============================================================================
// AUTHORIZE (CIT — transaction_type = D or P)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchantePaymentsRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_amount: StringMajorUnit,
    pub currency_code: String,
    pub card_number: domain_types::payment_method_data::RawCardNumber<T>,
    pub card_exp_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    pub moto_ecommerce_ind: String,
    pub account_data_source: String,
    pub client_reference_number: String,
    pub retry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_street_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_zip: Option<String>,
    /// Set to "Y" to have Merchante return a permanent card_id on the response
    /// (used when the caller wants to reuse the card for MIT later on).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_card: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MerchantePaymentsRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;

        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Confirm the currency is supported by StringMajorUnit conversion \
                         and that the minor_amount fits the currency's exponent."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Merchante Authorize: failed to convert PaymentsAuthorizeData.minor_amount \
                         into Merchante's decimal-string amount (StringMajorUnit)."
                            .to_string(),
                    ),
                },
            })?;

        let card =
            match &router_data.request.payment_method_data {
                PaymentMethodData::Card(card) => card,
                _ => return Err(Report::new(IntegrationError::NotSupported {
                    message: "Only raw card payments are supported by Merchante".to_string(),
                    connector: "merchante",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Route non-card payment methods to a different connector; \
                             Merchante's raw-card API accepts card_number + card_exp_date only."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Merchante Authorize: received PaymentMethodData variant other than \
                             Card. Wallets (Apple Pay / Google Pay decrypted), bank redirects, \
                             and BNPL are not wired for this connector."
                                .to_string(),
                        ),
                    },
                })),
            };

        let store_card = router_data
            .request
            .is_mandate_payment()
            .then(|| "Y".to_string());

        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        let card_exp_date = card
            .get_card_expiry_month_year_2_digit_with_delimiter(String::new())?
            .peek()
            .to_string();

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: sale_txn_type(router_data.request.is_auto_capture()).to_string(),
            transaction_amount: amount,
            currency_code: router_data.request.currency.iso_4217().to_string(),
            card_number: card.card_number.clone(),
            card_exp_date,
            cvv2: Some(card.card_cvc.clone()),
            moto_ecommerce_ind: ECOM_INDICATOR_DEFAULT.to_string(),
            account_data_source: ACCOUNT_DATA_SOURCE_KEYED.to_string(),
            client_reference_number: reference.clone(),
            retry_id: truncate_retry_id(reference),
            invoice_number: Some(reference.clone()),
            cardholder_street_address: None,
            cardholder_zip: None,
            store_card,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = MerchantePaymentStatus::from(&item.response)
            .to_attempt_status(item.router_data.request.is_auto_capture());

        if item.response.is_approved() {
            let mandate_reference = item.response.card_id.clone().map(|card_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(card_id),
                    payment_method_id: None,
                    mandate_metadata: None,
                    connector_mandate_request_reference_id: None,
                })
            });

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item.response.payment_account_reference.clone(),
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// PSYNC / RSYNC — Inquiry (transaction_type = I, keyed on retry_id)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteSyncRequest {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub retry_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for MerchanteSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: TXN_TYPE_INQUIRY.to_string(),
            retry_id: truncate_retry_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
        })
    }
}

impl TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = MerchantePaymentStatus::from(&item.response)
            .to_attempt_status(item.router_data.request.is_auto_capture());

        if item.response.is_approved() {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item.response.payment_account_reference.clone(),
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for MerchanteSyncRequest
{
    type Error = Report<IntegrationError>;
    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;
        // Use the same connector_request_reference_id that was sent as retry_id
        // on the original Refund — Merchante's Inquiry looks up by that field.
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;
        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: TXN_TYPE_INQUIRY.to_string(),
            retry_id: truncate_retry_id(reference),
        })
    }
}

impl TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: item.response.transaction_id.clone(),
                    refund_status: RefundStatus::from(MerchantePaymentStatus::from(&item.response)),
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// CAPTURE (Settle, transaction_type = S)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteCaptureRequest {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_amount: StringMajorUnit,
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    pub retry_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for MerchanteCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;
        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Confirm the capture currency is supported by StringMajorUnit \
                         conversion and that minor_amount_to_capture fits the currency's exponent."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Merchante Capture (transaction_type=S): failed to convert \
                         PaymentsCaptureData.minor_amount_to_capture into Merchante's \
                         decimal-string amount (StringMajorUnit)."
                            .to_string(),
                    ),
                },
            })?;

        let transaction_id = router_data.request.get_connector_transaction_id()?;
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: TXN_TYPE_SETTLE.to_string(),
            transaction_amount: amount,
            transaction_id,
            invoice_number: Some(reference.clone()),
            retry_id: truncate_retry_id(reference),
        })
    }
}

impl TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = if item.response.is_approved() {
            AttemptStatus::Charged
        } else {
            AttemptStatus::Failure
        };

        if item.response.is_approved() {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item.response.payment_account_reference.clone(),
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// VOID (transaction_type = V)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteVoidRequest {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_id: String,
    pub retry_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for MerchanteVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: TXN_TYPE_VOID.to_string(),
            transaction_id: router_data.request.connector_transaction_id.clone(),
            retry_id: truncate_retry_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            ),
        })
    }
}

impl TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = if item.response.is_approved() {
            AttemptStatus::Voided
        } else {
            AttemptStatus::VoidFailed
        };

        if item.response.is_approved() {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// REFUND (transaction_type = U — full or partial)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteRefundRequest {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_id: String,
    /// Omit for full refund; include for partial.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_amount: Option<StringMajorUnit>,
    pub retry_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for MerchanteRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;

        // Full refund when refund_amount == payment_amount; partial otherwise.
        let is_full_refund =
            router_data.request.minor_refund_amount == router_data.request.minor_payment_amount;

        let amount = if is_full_refund {
            None
        } else {
            Some(
                connector
                    .amount_converter
                    .convert(
                        router_data.request.minor_refund_amount,
                        router_data.request.currency,
                    )
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Confirm the refund currency matches the original payment and is \
                                 supported by StringMajorUnit conversion."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Merchante Refund (transaction_type=U, partial): failed to convert \
                                 RefundsData.minor_refund_amount into Merchante's decimal-string \
                                 amount."
                                    .to_string(),
                            ),
                        },
                    })?,
            )
        };

        // Use connector_request_reference_id (always populated) as the retry_id
        // seed; refund_id is Option and may not carry.
        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: TXN_TYPE_REFUND.to_string(),
            transaction_id: router_data.request.connector_transaction_id.clone(),
            transaction_amount: amount,
            retry_id: truncate_retry_id(reference),
        })
    }
}

impl TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: item.response.transaction_id.clone(),
                    refund_status: RefundStatus::from(MerchantePaymentStatus::from(&item.response)),
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// REPEAT PAYMENT (MIT — stored card_id + cit_mit_indicator)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteRepeatPaymentRequest {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_amount: StringMajorUnit,
    pub currency_code: String,
    pub card_id: String,
    pub moto_ecommerce_ind: String,
    pub account_data_source: String,
    pub card_on_file: String,
    pub merchant_initiated: String,
    pub cit_mit_indicator: String,
    pub client_reference_number: String,
    pub retry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MerchanteRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let connector = item.connector;

        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;
        let amount = connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Confirm the MIT charge currency is supported by StringMajorUnit \
                         conversion and that minor_amount fits the currency's exponent."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Merchante RepeatPayment (MIT): failed to convert \
                         RepeatPaymentData.minor_amount into Merchante's decimal-string amount."
                            .to_string(),
                    ),
                },
            })?;

        let card_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(m) => {
                m.get_connector_mandate_id().ok_or_else(|| {
                    Report::new(IntegrationError::NotSupported {
                        message: "Merchante MIT requires a connector_mandate_id".to_string(),
                        connector: "merchante",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Populate MandateReference.connector_mandate_id with the card_id \
                                 returned from a prior Merchante SetupMandate / off-session \
                                 Authorize (store_card=Y)."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Merchante RepeatPayment: MandateReferenceId::ConnectorMandateId \
                                 was present but its inner connector_mandate_id was None."
                                    .to_string(),
                            ),
                        },
                    })
                })?
            }
            _ => {
                return Err(Report::new(IntegrationError::NotSupported {
                    message:
                        "Merchante MIT only supports ConnectorMandateId — network mandates not \
                         supported"
                            .to_string(),
                    connector: "merchante",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Generate a Merchante card_id via SetupMandate or an off-session \
                             Authorize before charging; network-scheme mandate IDs \
                             (NetworkMandateId / NetworkTokenWithNTI) aren't accepted by \
                             Merchante."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Merchante RepeatPayment: received MandateReferenceId variant \
                             other than ConnectorMandateId (NetworkMandateId / \
                             NetworkTokenWithNTI). Merchante's cit_mit_indicator flow needs \
                             its own stored card_id token."
                                .to_string(),
                        ),
                    },
                }))
            }
        };

        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            transaction_type: sale_txn_type(router_data.request.is_auto_capture()).to_string(),
            transaction_amount: amount,
            currency_code: router_data.request.currency.iso_4217().to_string(),
            card_id,
            moto_ecommerce_ind: ECOM_INDICATOR_RECURRING_MOTO.to_string(),
            account_data_source: ACCOUNT_DATA_SOURCE_COF.to_string(),
            card_on_file: "Y".to_string(),
            merchant_initiated: "Y".to_string(),
            cit_mit_indicator: CIT_MIT_UNSCHEDULED.to_string(),
            client_reference_number: reference.clone(),
            retry_id: truncate_retry_id(reference),
            invoice_number: Some(reference.clone()),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = MerchantePaymentStatus::from(&item.response)
            .to_attempt_status(item.router_data.request.is_auto_capture());

        if item.response.is_approved() {
            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item.response.payment_account_reference.clone(),
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}

// ============================================================================
// SETUP MANDATE (Verify $0 with store_card=Y, transaction_type = A)
// ============================================================================

#[derive(Debug, Serialize)]
pub struct MerchanteSetupMandateRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub profile_id: Secret<String>,
    pub profile_key: Secret<String>,
    pub transaction_type: String,
    pub transaction_amount: StringMajorUnit,
    pub currency_code: String,
    pub card_number: domain_types::payment_method_data::RawCardNumber<T>,
    pub card_exp_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv2: Option<Secret<String>>,
    pub moto_ecommerce_ind: String,
    pub account_data_source: String,
    pub store_card: String,
    pub client_reference_number: String,
    pub retry_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        MerchanteRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for MerchanteSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: MerchanteRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth = MerchanteAuthType::try_from(&router_data.connector_config)?;

        let card = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            _ => {
                return Err(Report::new(IntegrationError::NotSupported {
                    message: "Only raw card payments are supported by Merchante".to_string(),
                    connector: "merchante",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Route non-card payment methods to a different connector; \
                             Merchante's tokenization endpoint accepts card_number + \
                             card_exp_date only."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Merchante SetupMandate (transaction_type=A with store_card=Y): \
                             received PaymentMethodData variant other than Card. Merchante \
                             tokenises raw cards only."
                                .to_string(),
                        ),
                    },
                }))
            }
        };

        let reference = &router_data
            .resource_common_data
            .connector_request_reference_id;

        Ok(Self {
            profile_id: auth.profile_id,
            profile_key: auth.profile_key,
            // Verify (A) validates the card without settling it; combined with
            // store_card=Y this returns a permanent card_id for later MIT calls.
            transaction_type: TXN_TYPE_VERIFY.to_string(),
            // Zero-value verify is USD-only per the Merchante docs; use the
            // amount converter's zero to stay consistent with its formatting.
            transaction_amount: StringMajorUnit::zero(),
            currency_code: router_data.request.currency.iso_4217().to_string(),
            card_number: card.card_number.clone(),
            card_exp_date: card
                .get_card_expiry_month_year_2_digit_with_delimiter(String::new())?
                .peek()
                .to_string(),
            cvv2: Some(card.card_cvc.clone()),
            moto_ecommerce_ind: ECOM_INDICATOR_DEFAULT.to_string(),
            account_data_source: ACCOUNT_DATA_SOURCE_KEYED.to_string(),
            store_card: "Y".to_string(),
            client_reference_number: reference.clone(),
            retry_id: truncate_retry_id(reference),
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<MerchantePaymentResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<MerchantePaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if item.response.is_approved() {
            let mandate_reference = item.response.card_id.clone().map(|card_id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(card_id),
                    payment_method_id: None,
                    mandate_metadata: None,
                    connector_mandate_request_reference_id: None,
                })
            });

            Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.transaction_id.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(item.response.transaction_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: item.response.payment_account_reference.clone(),
                }),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Charged,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            })
        } else {
            Ok(Self {
                response: Err(item.response.to_error_response(item.http_code)),
                ..item.router_data
            })
        }
    }
}
