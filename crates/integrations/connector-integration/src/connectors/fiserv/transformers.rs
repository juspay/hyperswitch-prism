use crate::{connectors::fiserv::FiservRouterData, types::ResponseRouterData};
use common_enums::enums;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize, Serializer};

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> Serialize
    for FiservCheckoutChargesRequest<T>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Checkout(inner) => inner.serialize(serializer),
            Self::Charges(inner) => inner.serialize(serializer),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    amount: Amount,
    merchant_details: MerchantDetails,
    #[serde(flatten)]
    checkout_charges_request: FiservCheckoutChargesRequest<T>,
}

#[derive(Debug)]
pub enum FiservCheckoutChargesRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Checkout(CheckoutPaymentsRequest),
    Charges(ChargesPaymentRequest<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutPaymentsRequest {
    order: FiservOrder,
    payment_method: FiservPaymentMethod,
    interactions: FiservInteractions,
    transaction_details: TransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservChannel {
    Web,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservPaymentInitiator {
    Merchant,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservCustomerConfirmation {
    ReviewAndPay,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservInteractions {
    channel: FiservChannel,
    customer_confirmation: FiservCustomerConfirmation,
    payment_initiator: FiservPaymentInitiator,
    return_urls: FiservReturnUrls,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservReturnUrls {
    success_url: String,
    cancel_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservPaymentMethod {
    provider: FiservWallet,
    #[serde(rename = "type")]
    wallet_type: FiservWalletType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservOrder {
    intent: FiservIntent,
}

#[derive(Debug, Serialize, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservIntent {
    Authorize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargesPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    source: Source<T>,
    transaction_interaction: Option<TransactionInteraction>,
    transaction_details: TransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservWallet {
    ApplePay,
    GooglePay,
    PayPal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FiservWalletType {
    PaypalWallet,
}

#[derive(Debug, Serialize)]
#[serde(tag = "sourceType")]
pub enum Source<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    PaymentCard {
        card: CardData<T>,
    },
    #[allow(dead_code)]
    GooglePay {
        data: Secret<String>,
        signature: Secret<String>,
        version: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardData<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
{
    pub card_data: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_code: Option<Secret<String>>,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayToken {
    pub signature: Secret<String>,
    pub signed_message: Secret<String>,
    pub protocol_version: String,
}

#[derive(Default, Debug, Serialize)]
pub struct Amount {
    pub total: FloatMajorUnit,
    pub currency: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_flag: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reversal_reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_type: Option<OperationType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OperationType {
    Create,
    Capture,
    Authorize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantDetails {
    pub merchant_id: Secret<String>,
    pub terminal_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInteraction {
    pub origin: TransactionInteractionOrigin,
    pub eci_indicator: TransactionInteractionEciIndicator,
    pub pos_condition_code: TransactionInteractionPosConditionCode,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionInteractionOrigin {
    #[default]
    Ecom,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionInteractionEciIndicator {
    #[default]
    ChannelEncrypted,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionInteractionPosConditionCode {
    #[default]
    CardNotPresentEcom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiservAuthType {
    pub api_key: Secret<String>,
    pub merchant_account: Secret<String>,
    pub api_secret: Secret<String>,
    pub terminal_id: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for FiservAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiserv {
                api_key,
                merchant_account,
                api_secret,
                terminal_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_account: merchant_account.to_owned(),
                api_secret: api_secret.to_owned(),
                terminal_id: terminal_id.clone(),
            }),
            _ => Err(report!(IntegrationError::FailedToObtainAuthType {
                context: Default::default()
            })),
        }
    }
}
#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservErrorResponse {
    pub details: Option<Vec<FiservErrorDetails>>,
    pub error: Option<Vec<FiservErrorDetails>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservErrorDetails {
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: Option<String>,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiservPaymentStatus {
    Succeeded,
    Failed,
    Captured,
    Declined,
    Voided,
    Authorized,
    #[default]
    Processing,
    Created,
}

impl From<FiservPaymentStatus> for enums::AttemptStatus {
    fn from(item: FiservPaymentStatus) -> Self {
        match item {
            FiservPaymentStatus::Captured | FiservPaymentStatus::Succeeded => Self::Charged,
            FiservPaymentStatus::Declined | FiservPaymentStatus::Failed => Self::Failure,
            FiservPaymentStatus::Processing => Self::Authorizing,
            FiservPaymentStatus::Voided => Self::Voided,
            FiservPaymentStatus::Authorized => Self::Authorized,
            FiservPaymentStatus::Created => Self::AuthenticationPending,
        }
    }
}

impl From<FiservPaymentStatus> for enums::RefundStatus {
    fn from(item: FiservPaymentStatus) -> Self {
        match item {
            FiservPaymentStatus::Captured
            | FiservPaymentStatus::Succeeded
            | FiservPaymentStatus::Authorized => Self::Success,
            FiservPaymentStatus::Declined | FiservPaymentStatus::Failed => Self::Failure,
            FiservPaymentStatus::Voided
            | FiservPaymentStatus::Processing
            | FiservPaymentStatus::Created => Self::Pending,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservPaymentsResponse {
    pub gateway_response: GatewayResponse,
}

// Create a new response type for Capture that's a clone of the payments response
// This resolves the naming conflict in the macro framework
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservCaptureResponse {
    pub gateway_response: GatewayResponse,
}

// Create a response type for Void
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservVoidResponse {
    pub gateway_response: GatewayResponse,
}

// Create Refund response type
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservRefundResponse {
    pub gateway_response: GatewayResponse,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(transparent)]
pub struct FiservSyncResponse {
    pub sync_responses: Vec<FiservPaymentsResponse>,
}

// Create a distinct type for RefundSync to avoid templating conflicts
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(transparent)]
pub struct FiservRefundSyncResponse {
    pub sync_responses: Vec<FiservPaymentsResponse>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    pub gateway_transaction_id: Option<String>,
    pub transaction_state: FiservPaymentStatus,
    pub transaction_processing_details: TransactionProcessingDetails,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionProcessingDetails {
    pub order_id: String,
    pub transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservCaptureRequest {
    pub amount: Amount,
    pub transaction_details: TransactionDetails,
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<FiservOrderRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservOrderRequest {
    order_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceTransactionDetails {
    pub reference_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservVoidRequest {
    pub transaction_details: TransactionDetails,
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservRefundRequest {
    pub amount: Amount,
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservSyncRequest {
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

// Create a distinct type for RefundSync to avoid templating conflicts
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiservRefundSyncRequest {
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

// Implementations for FiservRouterData - needed for the macro framework
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiservPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if item.router_data.resource_common_data.is_three_ds() {
            Err(error_stack::report!(IntegrationError::NotImplemented(
                "Cards 3DS".to_string(),
                Default::default()
            )))?
        }

        let auth: FiservAuthType = FiservAuthType::try_from(&item.router_data.connector_config)?;
        let total = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let amount = Amount {
            total,
            currency: item.router_data.request.currency.to_string(),
        };
        let merchant_details = MerchantDetails {
            merchant_id: auth.merchant_account,
            terminal_id: auth.terminal_id,
        };

        let checkout_charges_request = match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref ccard) => {
                Ok(FiservCheckoutChargesRequest::Charges(
                    ChargesPaymentRequest {
                        source: Source::PaymentCard {
                            card: CardData {
                                card_data: ccard.card_number.clone(),
                                expiration_month: ccard.card_exp_month.clone(),
                                expiration_year: ccard.get_expiry_year_4_digit(),
                                security_code: Some(ccard.card_cvc.clone()),
                            },
                        },
                        transaction_details: TransactionDetails {
                            capture_flag: Some(matches!(
                                item.router_data.request.capture_method,
                                Some(enums::CaptureMethod::Automatic)
                                    | Some(enums::CaptureMethod::SequentialAutomatic)
                                    | None
                            )),
                            reversal_reason_code: None,
                            merchant_transaction_id: Some(
                                item.router_data
                                    .resource_common_data
                                    .connector_request_reference_id
                                    .clone(),
                            ),
                            operation_type: None,
                        },
                        transaction_interaction: Some(TransactionInteraction {
                            //Payment is being made in online mode, card not present
                            origin: TransactionInteractionOrigin::Ecom,
                            // transaction encryption such as SSL/TLS, but authentication was not performed
                            eci_indicator: TransactionInteractionEciIndicator::ChannelEncrypted,
                            //card not present in online transaction
                            pos_condition_code:
                                TransactionInteractionPosConditionCode::CardNotPresentEcom,
                        }),
                    },
                ))
            }
            PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(error_stack::report!(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("fiserv"),
                    Default::default()
                )))
            }
        }?;

        Ok(Self {
            amount,
            checkout_charges_request,
            merchant_details,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for FiservCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;
        let auth: FiservAuthType = FiservAuthType::try_from(&router_data.connector_config)?;

        let merchant_details = MerchantDetails {
            merchant_id: auth.merchant_account.clone(),
            terminal_id: auth.terminal_id.clone(),
        };

        let total = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        let order_id = router_data.request.metadata.as_ref().and_then(|v| {
            let exposed = v.clone().expose();
            exposed
                .get("order_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

        Ok(Self {
            amount: Amount {
                total,
                currency: router_data.request.currency.to_string(),
            },
            order: Some(FiservOrderRequest { order_id }),
            transaction_details: TransactionDetails {
                capture_flag: Some(true),
                reversal_reason_code: None,
                merchant_transaction_id: Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                operation_type: Some(OperationType::Capture),
            },
            merchant_details,
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for FiservSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth: FiservAuthType = FiservAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            merchant_details: MerchantDetails {
                merchant_id: auth.merchant_account.clone(),
                terminal_id: None,
            },
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
            },
        })
    }
}

// Implementation for the Void request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FiservVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth: FiservAuthType = FiservAuthType::try_from(&router_data.connector_config)?;

        Ok(Self {
            merchant_details: MerchantDetails {
                merchant_id: auth.merchant_account.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
            transaction_details: TransactionDetails {
                capture_flag: None,
                reversal_reason_code: router_data.request.cancellation_reason.clone(),
                merchant_transaction_id: Some(
                    router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                ),
                operation_type: None,
            },
        })
    }
}

// Implementation for the Refund request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for FiservRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth: FiservAuthType = FiservAuthType::try_from(&router_data.connector_config)?;

        // Convert minor amount to float major unit
        let converter = FloatMajorUnitForConnector;
        let amount_major = converter
            .convert(
                router_data.request.minor_refund_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        Ok(Self {
            amount: Amount {
                total: amount_major,
                currency: router_data.request.currency.to_string(),
            },
            merchant_details: MerchantDetails {
                merchant_id: auth.merchant_account.clone(),
                terminal_id: auth.terminal_id.clone(),
            },
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.to_string(),
            },
        })
    }
}

// Implementation for the RefundSync request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiservRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for FiservRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiservRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth: FiservAuthType = FiservAuthType::try_from(&router_data.connector_config)?;
        Ok(Self {
            merchant_details: MerchantDetails {
                merchant_id: auth.merchant_account.clone(),
                terminal_id: None,
            },
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_refund_id.clone(),
            },
        })
    }
}

// Response handling TryFrom implementations for macro framework

// Standard payment response handling for Authorize flow
impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<FiservPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let gateway_resp = &response.gateway_response;
        let status = enums::AttemptStatus::from(gateway_resp.transaction_state.clone());

        // Update the status in router_data
        let mut router_data_out = router_data;
        router_data_out.resource_common_data.status = status;

        let response_payload = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                gateway_resp
                    .gateway_transaction_id
                    .clone()
                    .unwrap_or_else(|| {
                        gateway_resp
                            .transaction_processing_details
                            .transaction_id
                            .clone()
                    }),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(
                gateway_resp.transaction_processing_details.order_id.clone(),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        if status == enums::AttemptStatus::Failure || status == enums::AttemptStatus::Voided {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Payment status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: Some(status),
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Implementation for the Capture flow response
impl<F> TryFrom<ResponseRouterData<FiservCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let gateway_resp = &response.gateway_response;
        let status = enums::AttemptStatus::from(gateway_resp.transaction_state.clone());

        // Update the status in router_data
        let mut router_data_out = router_data;
        router_data_out.resource_common_data.status = status;

        let response_payload = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                gateway_resp
                    .gateway_transaction_id
                    .clone()
                    .unwrap_or_else(|| {
                        gateway_resp
                            .transaction_processing_details
                            .transaction_id
                            .clone()
                    }),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(
                gateway_resp.transaction_processing_details.order_id.clone(),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        if status == enums::AttemptStatus::Failure || status == enums::AttemptStatus::Voided {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Payment status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: Some(status),
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Implementation for the Void flow response
impl<F> TryFrom<ResponseRouterData<FiservVoidResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FiservVoidResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let gateway_resp = &response.gateway_response;
        let status = enums::AttemptStatus::from(gateway_resp.transaction_state.clone());

        // Update the status in router_data
        let mut router_data_out = router_data;
        router_data_out.resource_common_data.status = status;

        let response_payload = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                gateway_resp
                    .gateway_transaction_id
                    .clone()
                    .unwrap_or_else(|| {
                        gateway_resp
                            .transaction_processing_details
                            .transaction_id
                            .clone()
                    }),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(
                gateway_resp.transaction_processing_details.order_id.clone(),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        if status == enums::AttemptStatus::Failure {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Void status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: Some(status),
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Payment Sync response handling
impl<F> TryFrom<ResponseRouterData<FiservSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FiservSyncResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Get first transaction from array
        let fiserv_payment_response = response
            .sync_responses
            .first()
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "fiserv",
            ))
            .attach_printable("Fiserv Sync response array was empty")?;

        let gateway_resp = &fiserv_payment_response.gateway_response;
        let status = enums::AttemptStatus::from(gateway_resp.transaction_state.clone());

        // Update the status in router_data
        let mut router_data_out = router_data;
        router_data_out.resource_common_data.status = status;

        let response_payload = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                gateway_resp
                    .gateway_transaction_id
                    .clone()
                    .unwrap_or_else(|| {
                        gateway_resp
                            .transaction_processing_details
                            .transaction_id
                            .clone()
                    }),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(
                gateway_resp.transaction_processing_details.order_id.clone(),
            ),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        if status == enums::AttemptStatus::Failure || status == enums::AttemptStatus::Voided {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Payment status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: Some(status),
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Refund flow response handling
impl<F> TryFrom<ResponseRouterData<FiservRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FiservRefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let gateway_resp = &response.gateway_response;
        let refund_status = enums::RefundStatus::from(gateway_resp.transaction_state.clone());

        // Update the status in router_data
        let mut router_data_out = router_data;

        let response_payload = RefundsResponseData {
            connector_refund_id: gateway_resp
                .gateway_transaction_id
                .clone()
                .unwrap_or_else(|| {
                    gateway_resp
                        .transaction_processing_details
                        .transaction_id
                        .clone()
                }),
            refund_status,
            status_code: http_code,
        };

        if refund_status == enums::RefundStatus::Failure {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Refund status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Refund Sync response handling
impl<F> TryFrom<ResponseRouterData<FiservRefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiservRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        // Get first transaction from array
        let fiserv_payment_response = response
            .sync_responses
            .first()
            .ok_or(crate::utils::response_handling_fail_for_connector(
                item.http_code,
                "fiserv",
            ))
            .attach_printable("Fiserv Sync response array was empty")?;

        let gateway_resp = &fiserv_payment_response.gateway_response;
        let refund_status = enums::RefundStatus::from(gateway_resp.transaction_state.clone());

        // Update the router data
        let mut router_data_out = router_data;

        let response_payload = RefundsResponseData {
            connector_refund_id: gateway_resp
                .gateway_transaction_id
                .clone()
                .unwrap_or_else(|| {
                    gateway_resp
                        .transaction_processing_details
                        .transaction_id
                        .clone()
                }),
            refund_status,
            status_code: http_code,
        };

        if refund_status == enums::RefundStatus::Failure {
            router_data_out.response = Err(ErrorResponse {
                code: gateway_resp
                    .transaction_processing_details
                    .transaction_id
                    .clone(),
                message: format!("Refund status: {:?}", gateway_resp.transaction_state),
                reason: None,
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: gateway_resp.gateway_transaction_id.clone(),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            });
        } else {
            router_data_out.response = Ok(response_payload);
        }

        Ok(router_data_out)
    }
}

// Error response handling
impl<F, Req, Res> TryFrom<ResponseRouterData<FiservErrorResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, Req, Res>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<FiservErrorResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let error_details = response
            .error
            .as_ref()
            .or(response.details.as_ref())
            .and_then(|e| e.first());

        let message = error_details.map_or(NO_ERROR_MESSAGE.to_string(), |e| e.message.clone());
        let code = error_details
            .and_then(|e| e.code.clone())
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let reason = error_details.and_then(|e| e.field.clone());

        let mut router_data_out = router_data;
        router_data_out.response = Err(ErrorResponse {
            code,
            message,
            reason,
            status_code: http_code,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        });

        Ok(router_data_out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::print_stdout)]
mod tests {
    pub mod authorize {
        use std::{marker::PhantomData, str::FromStr};

        use common_utils::{request::RequestContent, types::MinorUnit};
        use domain_types::{
            connector_flow::Authorize,
            connector_types::{
                ConnectorEnum, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
            },
            payment_method_data::{DefaultPCIHolder, PaymentMethodData, RawCardNumber},
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };
        use hyperswitch_masking::{ExposeInterface, Secret};
        use interfaces::{
            connector_integration_v2::BoxedConnectorIntegrationV2, connector_types::BoxedConnector,
        };

        use crate::{connectors::Fiserv, types::ConnectorData};

        // Regression guard for fiserv Authorize: `merchantDetails.terminalId`
        // must be sourced from `ConnectorSpecificConfig::Fiserv.terminal_id`
        // (surfaced via `FiservAuthType::terminal_id`), never from per-payment
        // `request.metadata`. Pins against a PR-#723-class regression where
        // `FiservSessionObject` previously read `request.metadata` and emitted
        // `terminalId: null` whenever metadata was absent in shadow replay.
        #[test]
        fn terminal_id_sourced_from_auth_terminal_id() {
            let terminal_id = "10000001".to_string();
            let req: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    vault_headers: None,
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "pay_fiserv_regress".to_string(),
                    attempt_id: "attempt_fiserv_regress".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
                    auth_type: common_enums::AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: None,
                    connector_order_id: None,
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "conn_ref_fiserv_regress".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors {
                        fiserv: ConnectorParams {
                            base_url: "https://cert.api.fiservapps.com/".to_string(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    minor_amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Fiserv {
                    api_key: Secret::new("test_api_key".to_string()),
                    merchant_account: Secret::new("test_merchant_account".to_string()),
                    api_secret: Secret::new("test_api_secret".to_string()),
                    base_url: None,
                    terminal_id: Some(Secret::new(terminal_id.clone())),
                },
                request: PaymentsAuthorizeData {
                    authentication_data: None,
                    connector_testing_data: None,
                    access_token: None,
                    payment_method_data: PaymentMethodData::Card(
                        domain_types::payment_method_data::Card {
                            card_number: RawCardNumber(
                                cards::CardNumber::from_str("4111111111111111").unwrap(),
                            ),
                            card_cvc: Secret::new("123".to_string()),
                            card_exp_month: Secret::new("03".to_string()),
                            card_exp_year: Secret::new("2030".to_string()),
                            ..Default::default()
                        },
                    ),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: None,
                    customer_name: None,
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    capture_method: None,
                    integrity_object: None,
                    router_return_url: None,
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: None,
                    order_category: None,
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(common_enums::PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    // Deliberately None to pin that terminalId does NOT depend
                    // on request.metadata — that was the pre-PR-#723 regression class.
                    metadata: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    shipping_cost: None,
                    merchant_account_id: None,
                    merchant_config_currency: None,
                    all_keys_required: None,
                    customer_acceptance: None,
                    split_payments: None,
                    request_extended_authorization: None,
                    setup_mandate_details: None,
                    enable_overcapture: None,
                    connector_feature_data: None,
                    billing_descriptor: None,
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                    payment_channel: None,
                },
                response: Err(ErrorResponse::default()),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Fiserv::new());
            let connector_data = ConnectorData {
                connector,
                connector_name: ConnectorEnum::Fiserv,
            };

            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = connector_data.connector.get_connector_integration_v2();

            let request = connector_integration
                .build_request_v2(&req)
                .expect("fiserv Authorize build_request_v2 failed")
                .expect("fiserv Authorize build_request_v2 returned None");
            let body = request
                .body
                .as_ref()
                .expect("fiserv Authorize request body missing");

            // Use the same serialization path the HTTP layer uses on the wire.
            // masked_serialize() would render Secret<String> as
            // "*** alloc::string::String ***" — useless for this contract —
            // so we read the raw JSON via RequestContent::get_inner_value.
            let raw_json = match body {
                RequestContent::Json(_) => body.get_inner_value().expose(),
                other => panic!("expected JSON body, got {other:?}"),
            };
            let parsed: serde_json::Value =
                serde_json::from_str(&raw_json).expect("fiserv Authorize body is not valid JSON");

            assert_eq!(
                parsed["merchantDetails"]["terminalId"], terminal_id,
                "fiserv Authorize merchantDetails.terminalId must be sourced from \
                 ConnectorSpecificConfig::Fiserv.terminal_id (auth.terminal_id); \
                 regressed to: {}",
                parsed["merchantDetails"]["terminalId"],
            );
        }
    }
}
