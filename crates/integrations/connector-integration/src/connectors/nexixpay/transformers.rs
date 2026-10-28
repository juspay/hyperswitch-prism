use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, RefundStatus};
use common_utils::{
    self,
    crypto::{self, GenerateDigest},
    errors::CustomResult,
    types::{AmountConvertor, StringMinorUnit, StringMinorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PostAuthenticate, PreAuthenticate,
        RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, EventType, MandateReference,
        MandateReferenceId,
        NexixpayClientAuthenticationResponse as NexixpayClientAuthenticationResponseDomain,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_address::Address,
    payment_method_data::{
        GooglePayWalletData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes,
        WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    types::PaymentMethodDataType,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::Display;

// Import the wrapper type created by macros
use super::NexixpayRouterData;

const MAX_ORDER_ID_LENGTH: usize = 18;
/// Hex characters of the reference digest that end a shortened `order.orderId`, so two
/// references sharing a prefix (every retry attempt of one payment) stay distinct.
const ORDER_ID_DIGEST_LENGTH: usize = 9;
const MAX_CARD_HOLDER_LENGTH: usize = 255;
const MAX_ADDRESS_NAME_LENGTH: usize = 50;
const MAX_ADDRESS_STREET_LENGTH: usize = 50;
const MAX_ADDRESS_CITY_LENGTH: usize = 40;
const MAX_ADDRESS_POST_CODE_LENGTH: usize = 16;

/// Nexi XPay Payment API reference (every S2S endpoint this connector calls).
const NEXI_PAYMENT_API_DOC: &str =
    "https://developer.nexigroup.com/xpayglobal/en-EU/api/payment-api-v1/";
const NEXI_CONNECTOR: &str = "nexixpay";

/// Error context for a request-side refusal: what to change, where Nexi documents it,
/// and the connector-specific detail.
pub(super) fn nexi_ctx(
    suggested_action: &str,
    additional_context: impl Into<String>,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_owned()),
        doc_url: Some(NEXI_PAYMENT_API_DOC.to_owned()),
        additional_context: Some(additional_context.into()),
    }
}

/// Derives Nexi `order.orderId` from the merchant reference.
///
/// Nexi accepts up to 27 characters but strongly recommends at most 18 for circuit
/// compatibility, so a reference longer than 18 characters cannot be sent verbatim.
///
/// It is not truncated either. Hyperswitch sends the attempt id, which is
/// `{payment_id}_{attempt_count}` over a 24-character payment id -- so `pay_<20>_1` is 26
/// characters and its first 18 drop the `_1` that distinguishes one attempt from the next.
/// Every retry of a payment would derive the same orderId and Nexi would reject it with
/// GW0024 ("Order with same ID already exists"), making retries fail by construction.
///
/// A too-long reference therefore keeps a readable prefix and ends in a digest of the whole
/// reference, which keeps distinct references distinct while staying inside 18 characters.
/// It stays deterministic, so PreAuthenticate (`/orders/3steps/init`) and Authorize
/// (`/orders/3steps/payment`) still derive the same orderId from the same reference.
///
/// `payment_id` is never used: it is the constant IRRELEVANT_PAYMENT_ID on the gRPC path.
fn get_nexi_order_id(
    connector_request_reference_id: &str,
) -> CustomResult<String, IntegrationError> {
    if connector_request_reference_id.trim().is_empty() {
        return Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "connector_request_reference_id",
            context: nexi_ctx(
                "Send a merchant reference (merchant_transaction_id / merchant_order_id) on the request.",
                "Nexi order.orderId is derived from the merchant reference; an empty reference cannot identify the order.",
            ),
        }));
    }
    let reference = connector_request_reference_id.trim();
    if reference.chars().count() <= MAX_ORDER_ID_LENGTH {
        return Ok(reference.to_string());
    }
    let digest = crypto::Sha256
        .generate_digest(reference.as_bytes())
        .change_context(IntegrationError::RequestEncodingFailed {
            context: nexi_ctx(
                "Retry the request.",
                "Failed to derive the Nexi order.orderId digest from the merchant reference.",
            ),
        })?;
    let digest = hex::encode(digest);
    let prefix: String = reference
        .chars()
        .take(MAX_ORDER_ID_LENGTH - ORDER_ID_DIGEST_LENGTH)
        .collect();
    Ok(format!(
        "{prefix}{}",
        digest
            .get(..ORDER_ID_DIGEST_LENGTH)
            .unwrap_or(digest.as_str())
    ))
}

#[derive(Debug, Clone)]
pub struct NexixpayAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NexixpayAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nexixpay { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: nexi_ctx(
                        "Send x-connector-config with a Nexixpay entry carrying api_key.",
                        "Nexi authenticates every call with the X-Api-Key header; the connector config is not a Nexixpay config.",
                    ),
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayErrorBody {
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayErrorResponse {
    pub errors: Vec<NexixpayErrorBody>,
}

// ===== CONNECTOR METADATA STRUCTURE =====
// Used to pass data between 3DS flow steps (PreAuthenticate -> PostAuthenticate -> Authorize)
// Based on Hyperswitch implementation pattern

/// Payment flow intent for determining which operation ID to use in PSync
#[derive(Debug, Clone, Serialize, Deserialize, Display)]
pub enum NexixpayPaymentIntent {
    Capture,
    Cancel,
    Authorize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayConnectorMetaData {
    /// 3DS authentication result (CAVV, ECI, XID) from PostAuthenticate (/validate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d_s_auth_result: Option<NexixpayThreeDSAuthResult>,

    /// operationId from PreAuthenticate (/init) - used in Authorize (/payment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_operation_id: Option<String>,

    /// operationId from Capture operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_operation_id: Option<String>,

    /// operationId from Cancel/Void operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_operation_id: Option<String>,

    /// Payment flow type for PSync operations - required by Hyperswitch
    pub psync_flow: NexixpayPaymentIntent,
}

/// Fields a response mapper wants to merge into `connector_feature_data`.
#[derive(Debug, Clone)]
pub struct UpdateNexixpayConnectorMetaData {
    pub three_d_s_auth_result: Option<NexixpayThreeDSAuthResult>,
    pub authorization_operation_id: Option<String>,
    pub capture_operation_id: Option<String>,
    pub cancel_operation_id: Option<String>,
    pub psync_flow: Option<NexixpayPaymentIntent>,
    /// The payment was captured in the same operation (EXECUTED): the authorization
    /// operation is also the capture operation a Refund has to address.
    pub is_auto_capture: bool,
}

/// Typed merge of `connector_feature_data`, mirroring Hyperswitch `update_nexi_meta_data`:
/// ids already present are kept, absent ids are filled from `update`, an auto-captured
/// payment records its authorization operation as the capture operation, and `psync_flow`
/// is overwritten whenever the caller supplies one.
pub fn update_nexi_meta_data(
    existing: Option<serde_json::Value>,
    update: UpdateNexixpayConnectorMetaData,
) -> Result<serde_json::Value, error_stack::Report<ConnectorError>> {
    let existing = existing
        .map(serde_json::from_value::<NexixpayConnectorMetaData>)
        .transpose()
        .change_context(ConnectorError::response_handling_failed_http_status_unknown_with_context(
            Some(
                "nexixpay: inbound connector_feature_data is not the Nexixpay metadata shape \
                 (camelCase authorizationOperationId / captureOperationId / cancelOperationId / psyncFlow)"
                    .to_owned(),
            ),
        ))?;
    let (
        three_d_s_auth_result,
        authorization_operation_id,
        capture_operation_id,
        cancel_operation_id,
        psync_flow,
    ) = match existing {
        Some(meta) => (
            meta.three_d_s_auth_result,
            meta.authorization_operation_id,
            meta.capture_operation_id,
            meta.cancel_operation_id,
            Some(meta.psync_flow),
        ),
        None => (None, None, None, None, None),
    };
    let authorization_operation_id =
        authorization_operation_id.or(update.authorization_operation_id);
    let capture_operation_id = capture_operation_id.or(if update.is_auto_capture {
        authorization_operation_id.clone()
    } else {
        update.capture_operation_id
    });
    let merged = NexixpayConnectorMetaData {
        three_d_s_auth_result: three_d_s_auth_result.or(update.three_d_s_auth_result),
        authorization_operation_id,
        capture_operation_id,
        cancel_operation_id: cancel_operation_id.or(update.cancel_operation_id),
        psync_flow: update
            .psync_flow
            .or(psync_flow)
            .unwrap_or(NexixpayPaymentIntent::Authorize),
    };
    serde_json::to_value(merged).change_context(
        ConnectorError::response_handling_failed_http_status_unknown_with_context(Some(
            "nexixpay: failed to serialize the merged connector_feature_data".to_owned(),
        )),
    )
}

/// Parses the inbound `connector_feature_data` into the typed metadata shape.
/// Absent -> `Ok(None)`; present but not the Nexixpay shape -> `InvalidDataFormat`.
fn parse_nexi_meta_data(
    connector_feature_data: Option<&Secret<serde_json::Value>>,
) -> CustomResult<Option<NexixpayConnectorMetaData>, IntegrationError> {
    connector_feature_data
        .map(|data| serde_json::from_value::<NexixpayConnectorMetaData>(data.peek().clone()))
        .transpose()
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data",
            context: nexi_ctx(
                "Pass back the connector_feature_data returned by the previous Nexixpay call unchanged.",
                "Expected the Nexixpay metadata shape (camelCase authorizationOperationId, captureOperationId, cancelOperationId, psyncFlow).",
            ),
        })
}

pub fn get_payment_id(
    metadata: Option<serde_json::Value>,
    payment_intent: Option<NexixpayPaymentIntent>,
) -> CustomResult<String, IntegrationError> {
    const SUGGESTED_ACTION: &str = "Pass back the connector_feature_data returned by the previous Nexixpay call unchanged; it carries the Nexi operationIds.";
    let connector_metadata = metadata.ok_or_else(|| IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data",
        context: nexi_ctx(
            SUGGESTED_ACTION,
            "Nexi addresses GET /operations/{operationId} and the post-operations by operationId, which only connector_feature_data carries.",
        ),
    })?;
    let nexixpay_meta_data = serde_json::from_value::<NexixpayConnectorMetaData>(
        connector_metadata,
    )
    .change_context(IntegrationError::MissingRequiredField {
        field_name: "connector_feature_data",
        context: nexi_ctx(
            SUGGESTED_ACTION,
            "connector_feature_data is not the Nexixpay metadata shape (camelCase authorizationOperationId, captureOperationId, cancelOperationId, psyncFlow), so no operationId can be selected.",
        ),
    })?;
    let payment_flow = payment_intent.unwrap_or(nexixpay_meta_data.psync_flow);
    let (payment_id, field_name) = match payment_flow {
        NexixpayPaymentIntent::Cancel => (
            nexixpay_meta_data.cancel_operation_id,
            "connector_feature_data.cancelOperationId",
        ),
        NexixpayPaymentIntent::Capture => (
            nexixpay_meta_data.capture_operation_id,
            "connector_feature_data.captureOperationId",
        ),
        NexixpayPaymentIntent::Authorize => (
            nexixpay_meta_data.authorization_operation_id,
            "connector_feature_data.authorizationOperationId",
        ),
    };
    payment_id
        .filter(|operation_id| !operation_id.trim().is_empty())
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name,
                context: nexi_ctx(
                    SUGGESTED_ACTION,
                    format!("The {payment_flow} intent selects {field_name}, which connector_feature_data does not carry."),
                ),
            }
            .into()
        })
}

// Note: NexixpayThreeDSAuthResult is defined later in the file

// ===== AUTHORIZE FLOW STRUCTURES =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayCaptureType {
    Implicit,
    Explicit,
}

/// Maps the requested capture method onto Nexi `captureType` (Hyperswitch
/// `get_nexixpay_capture_type`): Manual -> EXPLICIT; Automatic, SequentialAutomatic or
/// unset -> IMPLICIT. Nexi has no multi-capture or scheduled-capture mode, so those are
/// refused before any HTTP call rather than silently auto-captured.
pub fn get_nexixpay_capture_type(
    capture_method: Option<common_enums::CaptureMethod>,
) -> CustomResult<NexixpayCaptureType, IntegrationError> {
    match capture_method {
        Some(common_enums::CaptureMethod::Manual) => Ok(NexixpayCaptureType::Explicit),
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(NexixpayCaptureType::Implicit),
        Some(
            unsupported @ (common_enums::CaptureMethod::ManualMultiple
            | common_enums::CaptureMethod::Scheduled),
        ) => Err(error_stack::report!(IntegrationError::NotSupported {
            message: format!("capture_method {unsupported}"),
            connector: NEXI_CONNECTOR,
            context: nexi_ctx(
                "Use capture_method MANUAL (captureType EXPLICIT) or AUTOMATIC (captureType IMPLICIT).",
                format!("Nexi captureType supports only IMPLICIT and EXPLICIT; {unsupported} has no Nexi equivalent."),
            ),
        })),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayRecurringAction {
    NoRecurring,
    ContractCreation,
    SubsequentPayment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContractType {
    MitUnscheduled,
    MitScheduled,
    Cit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceRequest {
    pub action: NexixpayRecurringAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_type: Option<ContractType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentsRequest {
    pub operation_id: String,
    pub order: NexixpayOrderData,
    pub card: NexixpayCardData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<NexixpayCaptureType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthData")]
    pub three_ds_auth_data: Option<NexixpayThreeDSAuthData>,
    pub recurrence: RecurrenceRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<NexixpayPaymentRequestActionType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayOrderData {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub customer_info: NexixpayCustomerInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCardData {
    pub pan: Secret<String>,
    #[serde(rename = "expiryDate")]
    pub expiry_date: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayThreeDSAuthData {
    /// PaRes (3DS 1) / CRes (3DS 2) as the ACS returned it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthResponse")]
    pub three_d_s_auth_response: Option<Secret<String>>,
    /// The CAVV / AAV proving the cardholder authenticated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<Secret<String>>,
}

// Request transformer implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // Nexi's server-to-server API carries cards only; every other payment method
        // (wallets, network tokens, NTID cards, bank methods) has no request shape here.
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(error_stack::report!(IntegrationError::NotImplemented(
                format!("Payment method {:?}", PaymentMethodDataType::from(payment_method_data.clone())),
                nexi_ctx(
                    "Use a card payment method; Nexi wallets and APMs are only available through the Hosted Payment Page.",
                    "Nexi /orders/3steps/payment accepts only a card object (pan, expiryDate, cvv).",
                ),
            )))?,
        };

        // G-ThreeDS-01: Nexi documents no non-3DS e-commerce S2S card endpoint, so a
        // no-3DS card payment is refused (Hyperswitch: "No threeds is not supported").
        if item.resource_common_data.auth_type != common_enums::AuthenticationType::ThreeDs {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "No threeds".to_string(),
                connector: NEXI_CONNECTOR,
                context: nexi_ctx(
                    "Send auth_type THREE_DS; Nexi card payments run through the 3-step 3DS flow.",
                    "Nexi exposes no non-3DS e-commerce server-to-server card payment endpoint.",
                ),
            }))?
        }

        // G-ThreeDS-02: capture method must map onto Nexi captureType.
        let capture_type = get_nexixpay_capture_type(item.request.capture_method)?;

        // G-ThreeDS-03: /orders/3steps/payment links to the init through its operationId.
        // The id rides connector_feature_data (Hyperswitch path, frictionless composite
        // path) or authentication_data.transaction_id (PostAuthenticate on the composite
        // path); both carry the same init operationId. Without one there is no Nexi init
        // to complete, and that is the only case refused here.
        //
        // Externally produced 3DS results are *dropped*, not refused. Every threeDSAuthData
        // value Nexi accepts is its own validation output, and it documents no field for an
        // externally obtained CAVV/ECI/XID/dsTransId. Nexi's own PostAuthenticate token never
        // carries a DS or ACS transaction id (see the PostAuthenticate response mapping), so
        // `authentication_data` that does carry one came from an external 3DS server: the
        // filter below discards it, its transaction id never becomes the operationId and its
        // CAVV is never forwarded. A request that still has a Nexi init operationId in
        // `connector_feature_data` therefore proceeds on Nexi's own authentication, with the
        // external values ignored; only one with no operationId at all is rejected.
        let nexi_authentication_data = item
            .request
            .authentication_data
            .as_ref()
            .filter(|data| data.ds_trans_id.is_none() && data.acs_transaction_id.is_none());
        let operation_id = parse_nexi_meta_data(
            item.resource_common_data.connector_feature_data.as_ref(),
        )?
        .and_then(|meta| meta.authorization_operation_id)
        .or_else(|| {
            nexi_authentication_data
                .and_then(|authentication_data| authentication_data.transaction_id.clone())
        })
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.authorizationOperationId",
            context: nexi_ctx(
                "Run PreAuthenticate (/orders/3steps/init) first and pass its connector_feature_data, or the PostAuthenticate authentication_data, to Authorize.",
                "Nexi /orders/3steps/payment requires the operationId returned by /orders/3steps/init. External 3DS (authentication_data cavv/eci/ds_trans_id obtained outside a Nexi 3steps init) is not supported by Nexi, so those values are ignored rather than forwarded and cannot stand in for the init.",
            ),
        })?;

        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        // PaRes posted back to ReturnUrl by the ACS, carried forward from the redirect.
        let pa_res = item
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect| redirect.payload.as_ref())
            .map(|payload| {
                serde_json::from_value::<NexixpayRedirectPayload>(payload.peek().clone())
            })
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "redirect_response.payload",
                context: nexi_ctx(
                    "Forward the ACS return payload unchanged (PaRes, paymentId).",
                    "The redirect payload did not match the Nexi ACS return shape.",
                ),
            })?
            .and_then(|redirect_payload| redirect_payload.pa_res);

        // Nexi /orders/3steps/payment accepts only PaRes and the CAVV in threeDSAuthData
        // (Hyperswitch parity: no eci / xid).
        let three_ds_auth_data = Some(NexixpayThreeDSAuthData {
            three_d_s_auth_response: pa_res,
            authentication_value: nexi_authentication_data
                .and_then(|authentication_data| authentication_data.cavv.clone()),
        });

        let order_id =
            get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?;

        let order = NexixpayOrderData {
            order_id: order_id.clone(),
            amount: StringMinorUnitForConnector
                .convert(item.request.amount.amount, item.request.amount.currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: nexi_ctx(
                        "Send a minor-unit amount representable in the request currency.",
                        "Nexi order.amount is a string of minor units.",
                    ),
                })?,
            currency: item.request.amount.currency,
            description: item.resource_common_data.description.clone(),
            customer_info: build_customer_info(&item.resource_common_data)?,
        };

        // Setting up a stored credential: Nexi creates the contract on the payment call,
        // with the orderId as contractId (the Authorize proto carries no separate
        // mandate request reference).
        let recurrence =
            if item.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession) {
                RecurrenceRequest {
                    action: NexixpayRecurringAction::ContractCreation,
                    contract_id: Some(Secret::new(order_id)),
                    contract_type: Some(ContractType::MitUnscheduled),
                }
            } else {
                RecurrenceRequest {
                    action: NexixpayRecurringAction::NoRecurring,
                    contract_id: None,
                    contract_type: None,
                }
            };

        // A zero-amount payment is a card verification.
        let action_type = (item.request.amount.amount == common_utils::types::MinorUnit::zero())
            .then_some(NexixpayPaymentRequestActionType::Verify);

        Ok(Self {
            operation_id,
            order,
            card,
            capture_type: Some(capture_type),
            three_ds_auth_data,
            recurrence,
            action_type,
        })
    }
}

// ===== GOOGLE PAY (PSP DECRYPTION) REQUEST: POST /orders/googlepay =====

/// `googlePayPaymentData.paymentMethodData.tokenizationData.type`: Nexi decrypts the
/// token itself, so only the gateway tokenization is accepted (`DIRECT` "is not supported").
const NEXI_GOOGLE_PAY_TOKENIZATION_TYPE: &str = "PAYMENT_GATEWAY";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayRequest {
    pub google_pay_payment_data: NexixpayGooglePayPaymentData,
    pub order: NexixpayClientAuthOrder,
    pub payment_session: NexixpayGooglePaySession,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayPaymentData {
    pub email: common_utils::pii::Email,
    pub payment_method_data: NexixpayGooglePayMethodData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayMethodData {
    #[serde(rename = "type")]
    pub pm_type: String,
    pub description: String,
    pub info: NexixpayGooglePayInfo,
    pub tokenization_data: NexixpayGooglePayTokenizationData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayInfo {
    pub card_network: String,
    pub card_details: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayTokenizationData {
    pub token: Secret<String>,
    #[serde(rename = "type")]
    pub token_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePaySession {
    pub action_type: NexixpayPaymentRequestActionType,
    pub amount: StringMinorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<NexixpayCaptureType>,
    pub notification_url: String,
    pub result_url: String,
    pub cancel_url: String,
}

/// Body of the Authorize call: the 3-step card payment (`/orders/3steps/payment`) or the
/// Google Pay PSP-decryption payment (`/orders/googlepay`), chosen by payment method.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NexixpayAuthorizeRequest {
    ThreeSteps(Box<NexixpayPaymentsRequest>),
    GooglePay(Box<NexixpayGooglePayRequest>),
}

/// A URL Nexi requires to be https (`paymentSession.notificationUrl` / `resultUrl` /
/// `cancelUrl`): absent -> MissingRequiredField, not https -> InvalidDataFormat.
fn get_nexi_https_url(
    url: Option<&String>,
    field_name: &'static str,
    suggested_action: &str,
    nexi_field: &str,
) -> CustomResult<String, IntegrationError> {
    let url = url.ok_or_else(|| IntegrationError::MissingRequiredField {
        field_name,
        context: nexi_ctx(
            suggested_action,
            format!("Nexi /orders/googlepay requires {nexi_field} (an https URL)."),
        ),
    })?;
    let is_https = url::Url::parse(url)
        .map(|parsed| parsed.scheme() == "https")
        .unwrap_or(false);
    if !is_https {
        Err(error_stack::report!(IntegrationError::InvalidDataFormat {
            field_name,
            context: nexi_ctx(
                suggested_action,
                format!("Nexi /orders/googlepay accepts only an https URL in {nexi_field}."),
            ),
        }))?
    }
    Ok(url.to_owned())
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        &GooglePayWalletData,
    )> for NexixpayGooglePayRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        (item, google_pay): (
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            &GooglePayWalletData,
        ),
    ) -> Result<Self, Self::Error> {
        // Nexi decrypts the Google Pay token (PSP decryption); a token Hyperswitch already
        // decrypted cannot be sent: DIRECT tokenization "is not supported" and the card
        // object has no cryptogram / ECI field.
        let token = match &google_pay.tokenization_data {
            GpayTokenizationData::Encrypted(encrypted) => Secret::new(encrypted.token.clone()),
            GpayTokenizationData::Decrypted(_) => Err(error_stack::report!(
                IntegrationError::NotSupported {
                    message: "Google Pay decrypted (DIRECT) tokenization".to_string(),
                    connector: NEXI_CONNECTOR,
                    context: nexi_ctx(
                        "Send the encrypted Google Pay PAYMENT_GATEWAY token (gateway nexigtw) so Nexi decrypts it.",
                        "Nexi /orders/googlepay rejects DIRECT tokenization and the Nexi card object carries no cryptogram or ECI.",
                    ),
                }
            ))?,
        };

        // G-ThreeDS-02: capture method must map onto Nexi captureType.
        let capture_type = get_nexixpay_capture_type(item.request.capture_method)?;

        // G-ThreeDS-08: googlePayPaymentData.email is required.
        let email = item
            .request
            .email
            .clone()
            .or_else(|| item.resource_common_data.get_optional_billing_email())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "email",
                context: nexi_ctx(
                    "Send the customer email (or a billing address email) with the Google Pay payment.",
                    "Nexi /orders/googlepay requires googlePayPaymentData.email.",
                ),
            })?;

        // G-ThreeDS-09: paymentSession.notificationUrl is required and https.
        let notification_url = get_nexi_https_url(
            item.request.webhook_url.as_ref(),
            "webhook_url",
            "Send an https webhook_url; Nexi posts the Google Pay outcome to it.",
            "paymentSession.notificationUrl",
        )?;

        // G-ThreeDS-10: paymentSession.resultUrl / cancelUrl are https URLs.
        let return_url = get_nexi_https_url(
            item.request.router_return_url.as_ref(),
            "return_url",
            "Send an https return_url; Nexi returns the cardholder to it after the Google Pay flow.",
            "paymentSession.resultUrl / cancelUrl",
        )?;

        // G-ThreeDS-11: this Nexi flow is not recurring, and VERIFY on /orders/googlepay is
        // undocumented, so stored-credential setup and zero-amount verification fail closed.
        let is_zero_amount = item.request.amount.amount == common_utils::types::MinorUnit::zero();
        if item.request.setup_future_usage == Some(common_enums::FutureUsage::OffSession)
            || is_zero_amount
        {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Google Pay recurring / zero-amount verification".to_string(),
                connector: NEXI_CONNECTOR,
                context: nexi_ctx(
                    "Send a non-zero Google Pay payment without setup_future_usage OFF_SESSION; set up recurring payments with a card.",
                    "Nexi /orders/googlepay is documented as non-recurring and documents no VERIFY behaviour.",
                ),
            }))?
        }

        let amount = StringMinorUnitForConnector
            .convert(item.request.amount.amount, item.request.amount.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nexi_ctx(
                    "Send a minor-unit amount representable in the request currency.",
                    "Nexi order.amount and paymentSession.amount are strings of minor units.",
                ),
            })?;

        Ok(Self {
            google_pay_payment_data: NexixpayGooglePayPaymentData {
                email,
                payment_method_data: NexixpayGooglePayMethodData {
                    pm_type: google_pay.pm_type.clone(),
                    description: google_pay.description.clone(),
                    info: NexixpayGooglePayInfo {
                        card_network: google_pay.info.card_network.clone(),
                        card_details: google_pay.info.card_details.clone(),
                    },
                    tokenization_data: NexixpayGooglePayTokenizationData {
                        token,
                        token_type: NEXI_GOOGLE_PAY_TOKENIZATION_TYPE.to_string(),
                    },
                },
            },
            order: NexixpayClientAuthOrder {
                order_id: get_nexi_order_id(
                    &item.resource_common_data.connector_request_reference_id,
                )?,
                amount: amount.clone(),
                currency: item.request.amount.currency,
                description: item.resource_common_data.description.clone(),
            },
            payment_session: NexixpayGooglePaySession {
                action_type: NexixpayPaymentRequestActionType::Pay,
                amount,
                capture_type: Some(capture_type),
                notification_url,
                result_url: return_url.clone(),
                cancel_url: return_url,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &value.router_data.request.payment_method_data {
            PaymentMethodData::Card(_) => Ok(Self::ThreeSteps(Box::new(
                NexixpayPaymentsRequest::try_from(value)?,
            ))),
            PaymentMethodData::Wallet(WalletData::GooglePay(google_pay)) => {
                Ok(Self::GooglePay(Box::new(NexixpayGooglePayRequest::try_from(
                    (&value.router_data, google_pay.as_ref()),
                )?)))
            }
            // Nexi's server-to-server API carries cards and Google Pay (PSP decryption)
            // only; every other payment method has no request shape here.
            payment_method_data => Err(error_stack::report!(IntegrationError::NotImplemented(
                format!("Payment method {:?}", PaymentMethodDataType::from(payment_method_data.clone())),
                nexi_ctx(
                    "Use a card or an encrypted Google Pay token; other Nexi wallets and APMs are only available through the Hosted Payment Page.",
                    "Nexi server-to-server payments accept a card (/orders/3steps/payment) or a Google Pay PAYMENT_GATEWAY token (/orders/googlepay) only.",
                ),
            ))),
        }
    }
}

// ===== RESPONSE STRUCTURES =====

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentsResponse {
    pub operation: NexixpayOperation,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayOperation {
    pub order_id: String,
    pub operation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_type: Option<String>,
    pub operation_result: NexixpayPaymentStatus,
    pub operation_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_circuit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_instrument_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_end_to_end_id: Option<String>,
    pub operation_amount: String,
    pub operation_currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data: Option<NexixpayAdditionalData>,
}

// Payment-specific operation result status enum
// CRITICAL: Matches Hyperswitch implementation exactly
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayPaymentStatus {
    Authorized,
    Executed,
    Declined,
    DeniedByRisk,
    ThreedsValidated,
    ThreedsFailed,
    Pending,
    Canceled,
    Voided,
    Refunded,
    Failed,
    /// An operationResult this connector does not know; never terminal.
    #[serde(other)]
    Unknown,
}

/// Maps Nexi `operationResult` onto the attempt status (Hyperswitch
/// `get_payment_attempt_status`). An unknown value retains the previous status.
pub fn get_payment_attempt_status(
    status: NexixpayPaymentStatus,
    prev_status: AttemptStatus,
) -> AttemptStatus {
    match status {
        // spec: ## Status Mappings (AUTHORIZED Payment authorized)
        NexixpayPaymentStatus::Authorized => AttemptStatus::Authorized,
        // spec: ## Status Mappings (EXECUTED Payment confirmed)
        NexixpayPaymentStatus::Executed => AttemptStatus::Charged,
        // spec: ## Status Mappings (DECLINED Declined by issuer)
        NexixpayPaymentStatus::Declined => AttemptStatus::Failure,
        // spec: ## Status Mappings (DENIED_BY_RISK Risk analysis negative)
        NexixpayPaymentStatus::DeniedByRisk => AttemptStatus::Failure,
        // spec: ## Status Mappings (THREEDS_VALIDATED 3DS OK or skipped)
        NexixpayPaymentStatus::ThreedsValidated => AttemptStatus::AuthenticationSuccessful,
        // spec: ## Status Mappings (THREEDS_FAILED 3DS failed / cancelled)
        NexixpayPaymentStatus::ThreedsFailed => AttemptStatus::Failure,
        // spec: ## Status Mappings (PENDING, used in authorization calls only)
        NexixpayPaymentStatus::Pending => AttemptStatus::AuthenticationPending,
        // spec: ## Status Mappings (CANCELED Canceled by cardholder)
        NexixpayPaymentStatus::Canceled => AttemptStatus::Voided,
        // spec: ## Status Mappings (VOIDED Full online reversal)
        NexixpayPaymentStatus::Voided => AttemptStatus::Voided,
        // spec: ## Status Mappings (REFUNDED Full or partial refunded)
        NexixpayPaymentStatus::Refunded => AttemptStatus::AutoRefunded,
        // spec: ## Status Mappings (FAILED Technical failure)
        NexixpayPaymentStatus::Failed => AttemptStatus::Failure,
        // spec: ## Status Mappings ((unknown) retain previous status)
        NexixpayPaymentStatus::Unknown => prev_status,
    }
}

/// `additionalData` of an operation. `authorizationStatus` is documented, so it is a typed
/// field rather than a map lookup; anything else Nexi sends alongside it is kept flattened so
/// the shape still round-trips.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayAdditionalData {
    /// The issuer response code (ISO, e.g. "101") Nexi reports on a failed operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_status: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Error response for a 2xx whose `operationResult` is a failure (Hyperswitch
/// `get_error_response`): code NO_ERROR_CODE, message and reason = the operationResult
/// wire name. The issuer response code Nexi reports in
/// `additionalData.authorizationStatus` (ISO code, e.g. "101") is surfaced as the
/// network decline code.
pub fn nexixpay_failure_response(
    operation: &NexixpayOperation,
    http_code: u16,
    attempt_status: AttemptStatus,
) -> ErrorResponse {
    nexixpay_operation_failure_response(
        &operation.operation_result,
        &operation.order_id,
        operation.additional_data.as_ref(),
        http_code,
        attempt_status,
    )
}

/// Field-level body of [`nexixpay_failure_response`], shared with the PSync mapper whose
/// `GET /operations/{operationId}` response is read into the minimal [`NexixpaySyncResponse`].
fn nexixpay_operation_failure_response(
    operation_result: &NexixpayPaymentStatus,
    order_id: &str,
    additional_data: Option<&NexixpayAdditionalData>,
    http_code: u16,
    attempt_status: AttemptStatus,
) -> ErrorResponse {
    let result = operation_result.to_string();
    ErrorResponse {
        code: common_utils::consts::NO_ERROR_CODE.to_string(),
        message: result.clone(),
        reason: Some(result),
        status_code: http_code,
        attempt_status: Some(FlowStatus::Payment(attempt_status)),
        connector_transaction_id: Some(order_id.to_owned()),
        network_decline_code: additional_data.and_then(|data| data.authorization_status.clone()),
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// Response transformer implementation
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let operation = &item.response.operation;
        let prev_status = item.router_data.resource_common_data.status;

        // A zero-amount verification answers AUTHORIZED; nothing remains to capture, so
        // it is terminal (Hyperswitch CompleteAuthorize special case).
        let status = if item.router_data.request.amount.amount
            == common_utils::types::MinorUnit::zero()
            && operation.operation_result == NexixpayPaymentStatus::Authorized
        {
            AttemptStatus::Charged
        } else {
            get_payment_attempt_status(operation.operation_result.clone(), prev_status)
        };

        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_failure_response(
                    operation,
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: Some(operation.operation_id.clone()),
                capture_operation_id: None,
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Authorize),
                is_auto_capture: operation.operation_result == NexixpayPaymentStatus::Executed,
            },
        )?;

        // The contract created on this payment (recurrence CONTRACT_CREATION) is
        // addressed by the orderId, which is the contractId the request sent.
        let mandate_reference = (item.router_data.request.setup_future_usage
            == Some(common_enums::FutureUsage::OffSession))
        .then(|| {
            Box::new(MandateReference {
                connector_mandate_id: Some(operation.order_id.clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(operation.order_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: Some(connector_metadata.clone()),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(operation.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: Some(Secret::new(connector_metadata)),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== GOOGLE PAY (PSP DECRYPTION) RESPONSE =====

/// `paymentFlow` of a `/orders/googlepay` 200: CRYPTOGRAM carries the payment operation,
/// PAN_ONLY needs the cardholder at the GDI iframe first.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayGooglePayFlow {
    Cryptogram,
    PanOnly,
    /// A paymentFlow this connector does not know.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayIframeField {
    #[serde(rename = "type")]
    pub field_type: Option<String>,
    pub src: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayIframe {
    #[serde(default)]
    pub fields: Vec<NexixpayGooglePayIframeField>,
    pub session_id: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayGooglePayResponse {
    pub payment_flow: NexixpayGooglePayFlow,
    pub operation: Option<NexixpayOperation>,
    pub iframe: Option<NexixpayGooglePayIframe>,
}

/// Response of the Authorize call. Untagged: the Google Pay shape is tried first and
/// requires `paymentFlow`, which a 3-step payment response never carries.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NexixpayAuthorizeResponse {
    GooglePay(NexixpayGooglePayResponse),
    ThreeSteps(NexixpayPaymentsResponse),
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let google_pay = match response {
            NexixpayAuthorizeResponse::ThreeSteps(payments_response) => {
                return Self::try_from(ResponseRouterData {
                    response: payments_response,
                    router_data,
                    http_code,
                })
            }
            NexixpayAuthorizeResponse::GooglePay(google_pay) => google_pay,
        };

        match (google_pay.payment_flow, google_pay.operation) {
            // PAN_ONLY: no money moved yet; the cardholder must load the GDI iframe.
            (NexixpayGooglePayFlow::PanOnly, _) => {
                let gdi_src = google_pay
                    .iframe
                    .as_ref()
                    .and_then(|iframe| {
                        iframe
                            .fields
                            .iter()
                            .find(|field| field.field_type.as_deref() == Some("GDI"))
                    })
                    .and_then(|field| field.src.clone())
                    .ok_or(crate::utils::response_deserialization_fail(
                        http_code,
                        "nexixpay: /orders/googlepay PAN_ONLY response has no iframe.fields[type GDI].src; the cardholder cannot be sent to the GDI verification.",
                    ))?;
                pending_google_pay_response(
                    router_data,
                    http_code,
                    AttemptStatus::AuthenticationPending,
                    Some(Box::new(
                        domain_types::router_response_types::RedirectForm::Form {
                            endpoint: gdi_src,
                            method: common_utils::request::Method::Get,
                            form_fields: HashMap::new(),
                        },
                    )),
                )
            }
            // CRYPTOGRAM: the operation is a payment operation exactly like a card
            // /orders/3steps/payment one (status, decline code, operationIds).
            (
                NexixpayGooglePayFlow::Cryptogram | NexixpayGooglePayFlow::Unknown,
                Some(operation),
            ) => Self::try_from(ResponseRouterData {
                response: NexixpayPaymentsResponse { operation },
                router_data,
                http_code,
            }),
            // No operation yet: the outcome arrives at notificationUrl / PSync.
            (NexixpayGooglePayFlow::Cryptogram | NexixpayGooglePayFlow::Unknown, None) => {
                pending_google_pay_response(router_data, http_code, AttemptStatus::Pending, None)
            }
        }
    }
}

/// A Google Pay response with no Nexi operation yet: the attempt is anchored to the
/// orderId the request sent.
fn pending_google_pay_response<T: PaymentMethodDataTypes>(
    router_data: RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    http_code: u16,
    status: AttemptStatus,
    redirection_data: Option<Box<domain_types::router_response_types::RedirectForm>>,
) -> Result<
    RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    error_stack::Report<ConnectorError>,
> {
    let order_id = get_nexi_order_id(
        &router_data
            .resource_common_data
            .connector_request_reference_id,
    )
    .change_context(crate::utils::response_handling_fail(
        http_code,
        "nexixpay: the Google Pay orderId cannot be derived from connector_request_reference_id.",
    ))?;
    Ok(RouterDataV2 {
        response: Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(order_id.clone()),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: Some(order_id),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        }),
        resource_common_data: PaymentFlowData {
            status,
            ..router_data.resource_common_data
        },
        ..router_data
    })
}

// ===== PSYNC FLOW STRUCTURES =====

// Empty request structure for GET-based sync
#[derive(Debug, Serialize)]
pub struct NexixpaySyncRequest;

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for NexixpaySyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync - operation_id extracted from URL
        Ok(Self)
    }
}

// PSync response structure - single operation object
// CRITICAL: GET /operations/{operation_id} returns a single operation, not an order with operations array
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpaySyncResponse {
    pub order_id: String,
    pub operation_id: String,
    pub operation_result: NexixpayPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_type: Option<NexixpayOperationType>,
    /// Carries `authorizationStatus`, the issuer response code surfaced on a failed operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data: Option<NexixpayAdditionalData>,
}

// Response transformer implementation for PSync
impl TryFrom<ResponseRouterData<NexixpaySyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NexixpaySyncResponse, Self>) -> Result<Self, Self::Error> {
        // An unknown operationResult retains the previous status (never terminal).
        let status = get_payment_attempt_status(
            item.response.operation_result.clone(),
            item.router_data.resource_common_data.status,
        );

        // A 2xx whose operationResult is a failure is an error response (Hyperswitch
        // PSync: Failure -> Err(get_error_response)).
        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_operation_failure_response(
                    &item.response.operation_result,
                    &item.response.order_id,
                    item.response.additional_data.as_ref(),
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let resource_id = ResponseId::ConnectorTransactionId(item.response.order_id.clone());

        let connector_metadata = item
            .router_data
            .request
            .connector_feature_data
            .as_ref()
            .map(|secret| secret.peek().clone());

        let mandate_reference = if item.router_data.request.is_mandate_payment() {
            item.router_data
                .request
                .mandate_reference
                .as_ref()
                .map(|m| {
                    Box::new(MandateReference {
                        connector_mandate_id: m.connector_mandate_id.clone(),
                        payment_method_id: m.payment_method_id.clone(),
                        connector_mandate_request_reference_id: m
                            .connector_mandate_request_reference_id
                            .clone(),
                        mandate_metadata: None,
                    })
                })
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference,
                connector_metadata,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.order_id.clone()),
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
    }
}

// ===== CAPTURE FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCaptureRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NexixpayCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // Convert amount - handle partial vs full capture
        let capture_amount = StringMinorUnitForConnector
            .convert(item.request.amount_to_capture.amount, item.request.amount_to_capture.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nexi_ctx(
                    "Send an amount_to_capture in minor units representable in the request currency.",
                    "Nexi /operations/{operationId}/captures takes amount as a string of minor units.",
                ),
            })?;

        Ok(Self {
            amount: capture_amount,
            currency: item.request.amount_to_capture.currency,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCaptureResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // spec: Status Mappings - the Capture POST response carries only operationId and
        // operationTime, no status, so the attempt stays Pending until PSync reads the
        // capture operation (Hyperswitch parity).
        //
        // connector_feature_data: record the capture operationId and move psyncFlow to
        // Capture (overwrite), so a later PSync reads the capture operation rather than
        // the authorization.
        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: None,
                capture_operation_id: Some(item.response.operation_id.clone()),
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Capture),
                is_auto_capture: false,
            },
        )?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // connector_transaction_id stays the Nexi orderId on every payment flow;
                // the capture operationId rides connector_feature_data.
                resource_id: item.router_data.request.connector_transaction_id.clone(),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(connector_metadata.clone()),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending, // Capture call does not return status in their response
                connector_feature_data: Some(Secret::new(connector_metadata)),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW STRUCTURES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRefundRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for NexixpayRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // The refund amount may be lower than the captured amount (partial refund); Nexi
        // rejects an amount above it with PS0168.
        let refund_amount = StringMinorUnitForConnector
            .convert(item.request.refund_amount.amount, item.request.refund_amount.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nexi_ctx(
                    "Send refund_amount in minor units of a currency Nexixpay supports.",
                    "Nexi POST /operations/{operationId}/refunds takes amount as a string in minor units.",
                ),
            })?;

        Ok(Self {
            amount: refund_amount,
            currency: item.request.refund_amount.currency,
            description: item.request.reason.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRefundResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // spec: Status Mappings — the refund POST response carries only operationId and
        // operationTime, no status. A 2xx means Nexi accepted the refund, not that it
        // completed, so the status is Pending; RSync resolves it via
        // GET /operations/{operationId} using the operationId returned here as
        // connector_refund_id.
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.operation_id.clone(),
                refund_status: RefundStatus::Pending,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// ===== VOID FLOW STRUCTURES =====
// CRITICAL: NexiXPay does NOT have a dedicated /cancels endpoint
// Void is implemented via POST /operations/{operationId}/refunds with full authorized amount
// This is a "refund before capture" which voids the authorization

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayVoidRequest {
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NexixpayVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // A Nexi void reverses the authorization in full: the amount must equal the
        // authorized amount (PS0082), so amount and currency are required and there is
        // no default (Hyperswitch MissingRequiredField parity).
        let void_amount =
            item.request
                .amount
                .as_ref()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: nexi_ctx(
                        "Send the full authorized amount on the Void request.",
                        "Nexi voids via POST /operations/{operationId}/refunds with the full authorized amount; a different amount is rejected with PS0082.",
                    ),
                })?;

        let currency = item
            .request
            .currency
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: nexi_ctx(
                    "Send the currency of the authorization on the Void request.",
                    "Nexi voids via POST /operations/{operationId}/refunds, whose body requires the authorization currency.",
                ),
            })?;

        let void_amount_string = StringMinorUnitForConnector
            .convert(void_amount.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nexi_ctx(
                    "Send a void amount representable in minor units of the given currency.",
                    format!("Failed to convert the void amount to Nexi minor units for currency {currency}."),
                ),
            })?;

        Ok(Self {
            amount: void_amount_string,
            currency,
            description: item.request.cancellation_reason.clone(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayVoidResponse {
    pub operation_id: String,
    pub operation_time: String,
}

impl TryFrom<ResponseRouterData<NexixpayVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NexixpayVoidResponse, Self>) -> Result<Self, Self::Error> {
        // spec: Status Mappings - the Void POST response carries only operationId and
        // operationTime, no status, so the attempt stays Pending until PSync reads the
        // cancel operation and reports VOIDED (Hyperswitch parity).
        //
        // connector_feature_data: record the cancel operationId and move psyncFlow to
        // Cancel (overwrite), so a later PSync reads the cancel operation rather than
        // the authorization.
        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: None,
                capture_operation_id: None,
                cancel_operation_id: Some(item.response.operation_id.clone()),
                psync_flow: Some(NexixpayPaymentIntent::Cancel),
                is_auto_capture: false,
            },
        )?;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // connector_transaction_id stays the Nexi orderId on every payment flow;
                // the cancel operationId rides connector_feature_data.
                resource_id: ResponseId::ConnectorTransactionId(
                    item.router_data.request.connector_transaction_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: Some(connector_metadata.clone()),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::Pending,
                connector_feature_data: Some(Secret::new(connector_metadata)),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC (RSync) FLOW STRUCTURES =====

// Empty request structure for GET-based refund sync
#[derive(Debug, Serialize)]
pub struct NexixpayRefundSyncRequest;

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for NexixpayRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync - connector_refund_id extracted from URL
        Ok(Self)
    }
}

// Refund-specific operation result status enum
// CRITICAL: Separate from payment status - matches Hyperswitch implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayRefundResultStatus {
    Pending,
    Voided,
    Refunded,
    Failed,
    Executed,
    /// An operationResult this connector does not know; never terminal.
    #[serde(other)]
    Unknown,
}

// RSync response structure - single operation object
// CRITICAL: GET /operations/{connector_refund_id} returns a single operation, not an order with operations array
// Only operationId and operationResult are read; orderId and operationType are optional so a
// response omitting them still resolves the refund (Hyperswitch NexixpayRSyncResponse parity).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRSyncResponse {
    pub order_id: Option<String>,
    pub operation_id: String,
    pub operation_result: NexixpayRefundResultStatus,
    pub operation_type: Option<NexixpayOperationType>,
}

/// Maps Nexi refund `operationResult` onto the refund status (Hyperswitch
/// `get_refund_status`). An unknown value retains the previous status.
pub fn get_refund_status(
    status: NexixpayRefundResultStatus,
    prev_status: RefundStatus,
) -> RefundStatus {
    match status {
        // spec: ## Status Mappings (PENDING -> refund Pending)
        NexixpayRefundResultStatus::Pending => RefundStatus::Pending,
        // spec: ## Status Mappings (VOIDED -> refund Success)
        NexixpayRefundResultStatus::Voided => RefundStatus::Success,
        // spec: ## Status Mappings (REFUNDED -> refund Success)
        NexixpayRefundResultStatus::Refunded => RefundStatus::Success,
        // spec: ## Status Mappings (FAILED -> refund Failure)
        NexixpayRefundResultStatus::Failed => RefundStatus::Failure,
        // spec: ## Status Mappings (EXECUTED -> refund Success)
        NexixpayRefundResultStatus::Executed => RefundStatus::Success,
        // spec: ## Status Mappings ((unknown) retain previous status)
        NexixpayRefundResultStatus::Unknown => prev_status,
    }
}

// Response transformer implementation for RSync
impl TryFrom<ResponseRouterData<NexixpayRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // CRITICAL: Map operation result to refund status using From trait
        // This addresses reviewer feedback:
        // - Checks operation_result status (Voided, Refunded, Executed, Pending, Failed)
        // - Does NOT assume success without checking detailed response
        let refund_status = get_refund_status(
            item.response.operation_result,
            item.router_data.request.refund_status,
        );

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.operation_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// ===== PRE-AUTHENTICATE FLOW STRUCTURES (Step 1 - Init 3DS) =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthenticateRequest {
    pub order: NexixpayPreAuthOrder,
    pub card: NexixpayCardData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<NexixpayRecurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_type: Option<NexixpayPaymentRequestActionType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthOrder {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    pub customer_info: NexixpayCustomerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayCustomerInfo {
    pub card_holder_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<NexixpayBillingAddress>,
    pub shipping_address: Option<NexixpayShippingAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayShippingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayRecurrence {
    pub action: NexixpayRecurringAction,
    pub contract_id: Option<Secret<String>>,
    pub contract_type: Option<ContractType>,
}

/// Fields of one Nexi `billingAddress` / `shippingAddress`, validated against Nexi's
/// documented limits.
struct ValidatedNexiAddress {
    name: Option<Secret<String>>,
    street: Option<Secret<String>>,
    city: Option<String>,
    post_code: Option<Secret<String>>,
    country: Option<common_enums::CountryAlpha3>,
}

fn check_nexi_field_length(
    field_name: String,
    received_length: usize,
    max_length: usize,
) -> CustomResult<(), IntegrationError> {
    if received_length > max_length {
        return Err(error_stack::report!(
            IntegrationError::MaxFieldLengthViolated {
                connector: NEXI_CONNECTOR.to_string(),
                context: nexi_ctx(
                    "Shorten the address field to the Nexi limit before retrying.",
                    format!("Nexi customerInfo limits {field_name} to {max_length} characters."),
                ),
                field_name,
                max_length,
                received_length,
            }
        ));
    }
    Ok(())
}

/// Mirrors Hyperswitch `get_validated_address_details_generic`: the address object is
/// sent whenever the payment carries one, name = full name, street = `line1, line2`,
/// country alpha-3, and every field over its Nexi limit is refused before any HTTP call.
fn validate_nexi_address(
    kind: &str,
    address: Option<&Address>,
) -> CustomResult<Option<ValidatedNexiAddress>, IntegrationError> {
    let Some(address) = address else {
        return Ok(None);
    };
    let details = address.address.as_ref();
    let full_name = address.get_optional_full_name();
    let street = match (
        details.and_then(|d| d.line1.clone()),
        details.and_then(|d| d.line2.clone()),
    ) {
        (Some(l1), Some(l2)) => Some(Secret::new(format!("{}, {}", l1.peek(), l2.peek()))),
        (Some(l1), None) => Some(l1),
        (None, Some(l2)) => Some(l2),
        (None, None) => None,
    };
    let city = details
        .and_then(|d| d.city.clone())
        .map(|city| city.expose());
    let post_code = details.and_then(|d| d.zip.clone());
    if let Some(name) = &full_name {
        check_nexi_field_length(
            format!("{kind}.address.first_name & {kind}.address.last_name"),
            name.peek().len(),
            MAX_ADDRESS_NAME_LENGTH,
        )?;
    }
    if let Some(street) = &street {
        check_nexi_field_length(
            format!("{kind}.address.line1 & {kind}.address.line2"),
            street.peek().len(),
            MAX_ADDRESS_STREET_LENGTH,
        )?;
    }
    if let Some(city) = &city {
        check_nexi_field_length(
            format!("{kind}.address.city"),
            city.len(),
            MAX_ADDRESS_CITY_LENGTH,
        )?;
    }
    if let Some(post_code) = &post_code {
        check_nexi_field_length(
            format!("{kind}.address.zip"),
            post_code.peek().len(),
            MAX_ADDRESS_POST_CODE_LENGTH,
        )?;
    }
    Ok(Some(ValidatedNexiAddress {
        name: full_name,
        street,
        city,
        post_code,
        country: details
            .and_then(|d| d.country)
            .map(common_enums::CountryAlpha2::from_alpha2_to_alpha3),
    }))
}

/// Builds Nexi `order.customerInfo` for every order-creating flow (Authorize,
/// PreAuthenticate, SetupMandate, RepeatPayment).
///
/// `cardHolderName` is the billing full name only (Hyperswitch parity, single source),
/// required whenever customerInfo is sent and at most 255 characters.
fn build_customer_info(
    flow_data: &PaymentFlowData,
) -> CustomResult<NexixpayCustomerInfo, IntegrationError> {
    let card_holder_name = flow_data
        .get_optional_billing_full_name()
        .ok_or(IntegrationError::MissingRequiredField {
        field_name: "billing.full_name",
        context: nexi_ctx(
            "Send the billing address first_name / last_name; Nexi uses it as the cardholder name.",
            "Nexi customerInfo.cardHolderName is required whenever customerInfo is sent.",
        ),
    })?;
    check_nexi_field_length(
        "billing.address.first_name & billing.address.last_name".to_string(),
        card_holder_name.peek().len(),
        MAX_CARD_HOLDER_LENGTH,
    )?;
    let billing_address =
        validate_nexi_address("billing", flow_data.get_optional_billing())?.map(|address| {
            NexixpayBillingAddress {
                name: address.name,
                street: address.street,
                city: address.city,
                post_code: address.post_code,
                country: address.country,
            }
        });
    let shipping_address = validate_nexi_address("shipping", flow_data.get_optional_shipping())?
        .map(|address| NexixpayShippingAddress {
            name: address.name,
            street: address.street,
            city: address.city,
            post_code: address.post_code,
            country: address.country,
        });
    Ok(NexixpayCustomerInfo {
        card_holder_name,
        billing_address,
        shipping_address,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NexixpayPaymentRequestActionType {
    Verify,
    Pay,
}

// PreAuthenticate Request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // /orders/3steps/init is card-only.
        let card_data = match item.request.payment_method_data.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method_data",
                context: nexi_ctx(
                    "Send the card on the PreAuthenticate request.",
                    "Nexi /orders/3steps/init requires card.pan, card.expiryDate and card.cvv.",
                ),
            },
        )? {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(error_stack::report!(IntegrationError::NotImplemented(
                format!("Payment method {:?} for 3DS", PaymentMethodDataType::from(payment_method_data.clone())),
                nexi_ctx(
                    "Use a card payment method; Nexi wallets and APMs are only available through the Hosted Payment Page.",
                    "Nexi /orders/3steps/init accepts only a card object.",
                ),
            )))?,
        };

        // G-ThreeDS-01: the 3-step init is the 3DS enrolment check; no-3DS is refused.
        if item.resource_common_data.auth_type != common_enums::AuthenticationType::ThreeDs {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "No threeds".to_string(),
                connector: NEXI_CONNECTOR,
                context: nexi_ctx(
                    "Send auth_type THREE_DS; Nexi card payments run through the 3-step 3DS flow.",
                    "Nexi exposes no non-3DS e-commerce server-to-server card payment endpoint.",
                ),
            }))?
        }

        // G-ThreeDS-04: ReturnUrl is a required field of the ACS form the init response
        // feeds; without it the cardholder cannot come back to complete the payment.
        if item.request.continue_redirection_url.is_none() {
            Err(error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: nexi_ctx(
                    "Send continue_redirection_url (the merchant complete-authorize URL).",
                    "The Nexi ACS form posts PaRes back to ReturnUrl; it is required (Hyperswitch requires complete_authorize_url).",
                ),
            }))?
        }

        // G-ThreeDS-05: order.currency is required on /orders/3steps/init.
        let currency = item
            .request
            .currency
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: nexi_ctx(
                    "Send amount.currency on the PreAuthenticate request.",
                    "Nexi /orders/3steps/init requires order.currency.",
                ),
            })?;

        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        let order = NexixpayPreAuthOrder {
            order_id: get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?,
            amount: StringMinorUnitForConnector
                .convert(item.request.amount.amount, currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: nexi_ctx(
                        "Send a minor-unit amount representable in the request currency.",
                        "Nexi order.amount is a string of minor units.",
                    ),
                })?,
            currency,
            customer_info: build_customer_info(&item.resource_common_data)?,
            description: item.resource_common_data.description.clone(),
        };

        // The init never creates a contract (Hyperswitch PreAuthenticate sends
        // NO_RECURRING); a stored credential is created on the payment call.
        let recurrence = Some(NexixpayRecurrence {
            action: NexixpayRecurringAction::NoRecurring,
            contract_id: None,
            contract_type: None,
        });

        // A zero-amount init is a card verification.
        let action_type = (item.request.amount.amount == common_utils::types::MinorUnit::zero())
            .then_some(NexixpayPaymentRequestActionType::Verify);

        Ok(Self {
            order,
            card,
            recurrence,
            action_type,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPreAuthenticateResponse {
    pub operation: NexixpayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSEnrollmentStatus")]
    pub three_ds_enrollment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthRequest")]
    pub three_ds_auth_request: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthUrl")]
    pub three_ds_auth_url: Option<String>,
}

// PreAuthenticate Response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;
        let status = get_payment_attempt_status(
            operation.operation_result.clone(),
            item.router_data.resource_common_data.status,
        );

        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_failure_response(
                    operation,
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // threeDSAuthUrl present -> the card is enrolled and the cardholder must be sent
        // to the ACS with the documented form (ThreeDsRequest, ReturnUrl, transactionId).
        // Absent -> frictionless: no redirect, and next_authentication_step goes straight
        // to Authorize.
        let redirection_data = match &response.three_ds_auth_url {
            Some(auth_url) => {
                let three_ds_request = response.three_ds_auth_request.clone().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                        "nexixpay: /orders/3steps/init returned threeDSAuthUrl without threeDSAuthRequest; the ACS form cannot be built.",
                    ),
                )?;
                let return_url = item
                    .router_data
                    .request
                    .continue_redirection_url
                    .as_ref()
                    .ok_or(crate::utils::response_handling_fail(
                        item.http_code,
                        "nexixpay: continue_redirection_url is required to build the ACS ReturnUrl form field.",
                    ))?;
                let form_fields = HashMap::from([
                    ("ThreeDsRequest".to_string(), three_ds_request),
                    ("ReturnUrl".to_string(), return_url.to_string()),
                    ("transactionId".to_string(), operation.operation_id.clone()),
                ]);
                Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Form {
                        endpoint: auth_url.clone(),
                        method: common_utils::request::Method::Post,
                        form_fields,
                    },
                ))
            }
            None => None,
        };

        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: Some(operation.operation_id.clone()),
                capture_operation_id: None,
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Authorize),
                is_auto_capture: false,
            },
        )?;

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                // Hyperswitch anchors the attempt to the orderId returned by /init.
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    operation.order_id.clone(),
                )),
                redirection_data,
                connector_response_reference_id: Some(operation.order_id.clone()),
                status_code: item.http_code,
                authentication_data: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: Some(Secret::new(connector_metadata)),
                preprocessing_id: Some(operation.operation_id.clone()),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== POST-AUTHENTICATE FLOW STRUCTURES (Step 3 - Validate 3DS) =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPostAuthenticateRequest {
    pub operation_id: String,
    #[serde(rename = "threeDSAuthResponse")]
    /// PaRes (3DS 1) / CRes (3DS 2) posted back by the ACS.
    pub three_ds_auth_response: Secret<String>,
}

// Redirect payload structure - returned from NexiXPay 3DS challenge
// Following Hyperswitch pattern for redirect response parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexixpayRedirectPayload {
    #[serde(rename = "PaRes")]
    pub pa_res: Option<Secret<String>>,
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
}

// PostAuthenticate Request transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;
        // G-ThreeDS-06: the validation call needs the PaRes the ACS posted back to
        // ReturnUrl. An absent payload, an empty payload (abandoned challenge) or a
        // payload without PaRes are all refused before any HTTP call.
        let missing_pa_res = || {
            IntegrationError::MissingRequiredField {
            field_name: "redirect_response.payload.PaRes",
            context: nexi_ctx(
                "Forward the ACS return payload (PaRes, paymentId) posted to the ReturnUrl unchanged.",
                "Nexi /orders/3steps/validation requires threeDSAuthResponse = the PaRes from the ACS.",
            ),
        }
        };
        let redirect_payload = item
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect_response| redirect_response.payload.as_ref())
            .map(|payload| {
                serde_json::from_value::<NexixpayRedirectPayload>(payload.peek().clone())
            })
            .transpose()
            .change_context(IntegrationError::InvalidDataFormat {
                field_name: "redirect_response.payload",
                context: nexi_ctx(
                    "Forward the ACS return payload (PaRes, paymentId) unchanged.",
                    "The redirect payload did not match the Nexi ACS return shape.",
                ),
            })?
            .ok_or_else(missing_pa_res)?;

        let three_ds_auth_response = redirect_payload.pa_res.ok_or_else(missing_pa_res)?;

        // operationId: the ACS returns it as paymentId; otherwise the init operationId
        // carried in connector_feature_data.
        let operation_id = match redirect_payload.payment_id {
            Some(payment_id) => payment_id,
            None => parse_nexi_meta_data(item.resource_common_data.connector_feature_data.as_ref())?
                .and_then(|meta| meta.authorization_operation_id)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.authorizationOperationId",
                    context: nexi_ctx(
                        "Forward the ACS paymentId, or pass the PreAuthenticate connector_feature_data.",
                        "Nexi /orders/3steps/validation requires the operationId returned by /orders/3steps/init.",
                    ),
                })?,
        };

        Ok(Self {
            operation_id,
            three_ds_auth_response,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPostAuthenticateResponse {
    pub operation: NexixpayOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "threeDSAuthResult")]
    pub three_ds_auth_result: Option<NexixpayThreeDSAuthResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayThreeDSAuthResult {
    /// The CAVV / AAV Nexi echoes back; masked like the value sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_value: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

// PostAuthenticate Response transformer
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;
        let status = get_payment_attempt_status(
            operation.operation_result.clone(),
            item.router_data.resource_common_data.status,
        );

        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_failure_response(
                    operation,
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Hyperswitch sends PostAuthenticate without connector_feature_data and reads the
        // returned feature data back, so the init operationId (the validated operation)
        // and the 3DS result are re-emitted here for the following Authorize.
        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: response.three_ds_auth_result.clone(),
                authorization_operation_id: Some(operation.operation_id.clone()),
                capture_operation_id: None,
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Authorize),
                is_auto_capture: false,
            },
        )?;

        let authentication_data = response.three_ds_auth_result.as_ref().map(|auth_result| {
            AuthenticationData {
                trans_status: auth_result
                    .status
                    .as_ref()
                    .and_then(|s| s.parse::<common_enums::TransactionStatus>().ok()),
                eci: auth_result.eci.clone(),
                cavv: auth_result.authentication_value.clone(),
                ucaf_collection_indicator: None,
                threeds_server_transaction_id: None,
                message_version: auth_result
                    .version
                    .as_ref()
                    .and_then(|v| v.parse::<common_utils::types::SemanticVersion>().ok()),
                ds_trans_id: None,
                acs_transaction_id: None,
                // The init operationId: Authorize (/orders/3steps/payment) needs it on the
                // composite path, where connector_feature_data is not forwarded.
                transaction_id: Some(operation.operation_id.clone()),
                exemption_indicator: None,
                network_params: None,
                created_at: None,
                challenge_code: None,
                challenge_cancel: None,
                challenge_code_reason: None,
                message_extension: None,
                authentication_type: None,
            }
        });

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: Some(operation.order_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: Some(Secret::new(connector_metadata)),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Nexixpay HPP (Hosted Payment Page) order for client-side SDK initialization.
/// The securityToken and hostedPage URL are returned to the frontend for
/// client-side redirect / hosted payment page initialization.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthRequest {
    pub order: NexixpayClientAuthOrder,
    pub payment_session: NexixpayPaymentSession,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthOrder {
    pub order_id: String,
    pub amount: StringMinorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayPaymentSession {
    pub action_type: String,
    pub amount: StringMinorUnit,
    pub recurrence: NexixpaySessionRecurrence,
    pub result_url: String,
    pub cancel_url: String,
    pub notification_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpaySessionRecurrence {
    pub action: NexixpayRecurringAction,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayClientAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NexixpayRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // MerchantAuthenticationFlowData carries no payment id; the orderId derives from the
        // request reference id like every other order-creating flow.
        let order_id = get_nexi_order_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        let amount = StringMinorUnitForConnector
            .convert(
                router_data.request.amount.amount,
                router_data.request.amount.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the request carries a valid minor-unit amount and a currency \
                         supported by Nexi XPay. Nexi `/orders/hpp` expects \
                         `paymentSession.amount` as a string of minor units (e.g. cents), so \
                         the source `MinorUnit` must be representable as an integer string."
                            .to_owned(),
                    ),
                    doc_url: Some("https://developer.nexi.it/en/api/post-orders-hpp".to_owned()),
                    additional_context: Some(format!(
                        "Failed to encode amount={:?} currency={:?} as a Nexi \
                         `paymentSession.amount` (string minor-units) for the /orders/hpp \
                         ClientAuthenticationToken flow.",
                        router_data.request.amount.amount, router_data.request.amount.currency,
                    )),
                },
            })?;

        let return_url = router_data.resource_common_data.return_url.clone().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Populate `return_url` on the PaymentCreate/Confirm request so Nexi's \
                         Hosted Payment Page can redirect the cardholder back to the merchant \
                         on both successful and cancelled payments. This connector reuses \
                         `return_url` for both `paymentSession.resultUrl` and \
                         `paymentSession.cancelUrl`."
                            .to_owned(),
                    ),
                    doc_url: Some("https://developer.nexi.it/en/api/post-orders-hpp".to_owned()),
                    additional_context: Some(
                        "Nexi XPay /orders/hpp marks both `paymentSession.resultUrl` and \
                         `paymentSession.cancelUrl` as REQUIRED; without `return_url` the \
                         hosted payment session (ClientAuthenticationToken flow) cannot be \
                         initialized."
                            .to_owned(),
                    ),
                },
            },
        )?;

        Ok(Self {
            order: NexixpayClientAuthOrder {
                order_id,
                amount: amount.clone(),
                currency: router_data.request.amount.currency,
                description: None,
            },
            payment_session: NexixpayPaymentSession {
                action_type: "PAY".to_string(),
                amount,
                recurrence: NexixpaySessionRecurrence {
                    action: NexixpayRecurringAction::NoRecurring,
                },
                result_url: return_url.clone(),
                cancel_url: return_url,
                notification_url: None,
            },
        })
    }
}

/// Nexixpay HPP order response containing securityToken and hostedPage URL for SDK initialization.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayClientAuthResponse {
    pub security_token: Secret<String>,
    pub hosted_page: String,
}

impl TryFrom<ResponseRouterData<NexixpayClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NexixpayClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Nexixpay(
                NexixpayClientAuthenticationResponseDomain {
                    security_token: response.security_token,
                    hosted_page: response.hosted_page,
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

// ===== SETUP MANDATE (Pay.SetupRecurring) FLOW STRUCTURES =====
//
// NexiXPay has no dedicated "tokenize the card without moving funds" endpoint.
// A stored credential is created by replaying a merchant-assigned `contractId`
// on `/orders/3steps/init` with `recurrence.action = CONTRACT_CREATION`; future
// CIT / MIT charges replay that same `contractId`.
//
// This mirrors hyperswitch, where SetupMandate reuses the Authorize "init"
// request (`NexixpayPaymentsRequest`) rather than a bespoke body. In prism the
// "init" request is `NexixpayPreAuthenticateRequest`, so SetupMandate reuses it
// verbatim (hence the type alias below). The only thing that makes it a *setup*
// call is that the `TryFrom` impl below always pins the recurrence to
// `CONTRACT_CREATION` + `MIT_UNSCHEDULED`; `actionType = VERIFY` is sent only
// when the setup amount is zero (card verification — no funds move).
//
// The `contractId` is sourced from `connector_request_reference_id` (prism's
// analog of hyperswitch's `connector_mandate_request_reference_id`, populated
// from the `merchant_recurring_payment_id` on the SetupRecurring request), and
// the SAME value is surfaced back as `mandate_reference.connector_mandate_id`
// (see the response transformer) so the orchestrator can replay it on a future
// RepeatPayment / MIT call. The Nexi `operationId` is NOT the mandate anchor —
// it is preserved on `connector_feature_data` / `preprocessing_id` purely so the
// downstream PostAuthenticate / PSync flows can drive the 3DS handshake.
pub type NexixpaySetupMandateRequest = NexixpayPreAuthenticateRequest;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpaySetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // Card is the only payment method supported by NexiXPay's SetupMandate
        // (the underlying /orders/3steps/init endpoint is card-only).
        let card_data = match &item.request.payment_method_data {
            PaymentMethodData::Card(card) => card,
            payment_method_data => Err(error_stack::report!(IntegrationError::NotImplemented(
                format!("Payment method {:?} for SetupMandate", PaymentMethodDataType::from(payment_method_data.clone())),
                nexi_ctx(
                    "Use a card payment method; Nexi wallets and APMs are only available through the Hosted Payment Page.",
                    "Nexi /orders/3steps/init (contract creation) accepts only a card object.",
                ),
            )))?,
        };

        // G-SetupMandate-01: the contract is created through the 3-step 3DS flow;
        // a no-3DS mandate setup is refused before any HTTP call (Hyperswitch parity).
        if item.resource_common_data.auth_type != common_enums::AuthenticationType::ThreeDs {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "No threeds is not supported".to_string(),
                connector: NEXI_CONNECTOR,
                context: nexi_ctx(
                    "Send auth_type THREE_DS; Nexi creates card contracts only through the 3-step 3DS flow.",
                    "Nexi exposes no non-3DS e-commerce server-to-server contract creation endpoint.",
                ),
            }))?
        }

        // G-SetupMandate-02: ReturnUrl of the ACS form the init response feeds; without
        // it the cardholder cannot come back to complete the contract.
        if item.request.complete_authorize_url.is_none() {
            Err(error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "complete_authorize_url",
                context: nexi_ctx(
                    "Send complete_authorize_url (the merchant complete-authorize URL).",
                    "The Nexi ACS form posts PaRes back to ReturnUrl; it is required (Hyperswitch requires complete_authorize_url).",
                ),
            }))?
        }

        let card = NexixpayCardData {
            pan: Secret::new(card_data.card_number.peek().to_string()),
            expiry_date: card_data
                .get_card_expiry_month_year_2_digit_with_delimiter("".to_string())?,
            cvv: Some(card_data.card_cvc.clone()),
        };

        // Cardholder name (billing full name) and validated addresses, shared with
        // every order-creating flow.
        let customer_info = build_customer_info(&item.resource_common_data)?;

        // Hyperswitch reuses the Authorize init body with the request amount; a
        // setup without an amount is a zero-amount card verification.
        //
        // The amount and its currency come from the same `Money` when one is sent, so the
        // pair cannot drift. `request.currency` is the fallback only for the zero-amount
        // case, where there is no `Money` to read a currency from.
        let (minor_amount, currency) = match item.request.amount.as_ref() {
            Some(money) => (money.amount, money.currency),
            None => (
                common_utils::types::MinorUnit::zero(),
                item.request.currency,
            ),
        };
        let order = NexixpayPreAuthOrder {
            order_id: get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?,
            amount: StringMinorUnitForConnector
                .convert(minor_amount, currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: nexi_ctx(
                        "Send a minor-unit amount representable in the request currency.",
                        "Nexi order.amount is a string of minor units.",
                    ),
                })?,
            currency,
            customer_info,
            description: item.resource_common_data.description.clone(),
        };

        // CONTRACT_CREATION + MIT_UNSCHEDULED — this is how Nexi registers a
        // stored credential. Nexi requires a merchant-assigned `contractId` on
        // the wire; we use the order id, which is derived from
        // `connector_request_reference_id` (== `merchant_recurring_payment_id`)
        // and is prism's analog of hyperswitch's `connector_mandate_request_reference_id`
        // (hyperswitch sources the SetupMandate contractId from that same
        // request reference — never from `mandate_id`, which is always absent on
        // a fresh setup). Nexi echoes this value back as `operation.order_id`,
        // and the response transformer surfaces it as
        // `mandate_reference.connector_mandate_id`, so the wire `contractId` and
        // the stored mandate anchor are guaranteed to be the same value — the
        // round-trip a future RepeatPayment / MIT call relies on.
        let recurrence = Some(NexixpayRecurrence {
            action: NexixpayRecurringAction::ContractCreation,
            contract_id: Some(Secret::new(order.order_id.clone())),
            contract_type: Some(ContractType::MitUnscheduled),
        });

        // VERIFY only on a zero-amount setup (card verification, no funds move);
        // Nexi rejects PAY with amount 0 (spec ### SetupMandate, UD-04).
        let action_type = (minor_amount == common_utils::types::MinorUnit::zero())
            .then_some(NexixpayPaymentRequestActionType::Verify);

        Ok(Self {
            order,
            card,
            recurrence,
            action_type,
        })
    }
}

// SetupMandate response shape is identical to the PreAuthenticate init
// response — same operation envelope, same optional 3DS auth payload — so we
// re-use the existing response body and only customise the response-side
// mapping to populate `mandate_reference.connector_mandate_id` with the
// `contractId` we created (echoed back as `operation.order_id`), which is the
// stable anchor a future RepeatPayment / MIT call replays via
// `recurrence.contractId`.
pub type NexixpaySetupMandateResponse = NexixpayPreAuthenticateResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpaySetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpaySetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let operation = &response.operation;

        // Same operationResult mapping as every payment flow (explicit arms, Unknown
        // retains the previous status). For a real card the init answers PENDING
        // (AuthenticationPending + ACS redirect form).
        let status = get_payment_attempt_status(
            operation.operation_result.clone(),
            item.router_data.resource_common_data.status,
        );

        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_failure_response(
                    operation,
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Typed merge so the PostAuthenticate / Authorize / PSync legs find the
        // init operationId.
        let connector_metadata = update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: Some(operation.operation_id.clone()),
                capture_operation_id: None,
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Authorize),
                is_auto_capture: false,
            },
        )?;

        // threeDSAuthUrl present -> the cardholder is sent to the ACS with the
        // documented form (ThreeDsRequest, ReturnUrl, transactionId). ReturnUrl is
        // `complete_authorize_url` (the continuation endpoint, Hyperswitch
        // `get_complete_authorize_url`), not `router_return_url`.
        let redirection_data = match &response.three_ds_auth_url {
            Some(auth_url) => {
                let three_ds_request = response.three_ds_auth_request.clone().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                        "nexixpay: /orders/3steps/init returned threeDSAuthUrl without threeDSAuthRequest; the ACS form cannot be built.",
                    ),
                )?;
                let return_url = item
                    .router_data
                    .request
                    .complete_authorize_url
                    .as_ref()
                    .ok_or(crate::utils::response_handling_fail(
                        item.http_code,
                        "nexixpay: complete_authorize_url is required to build the ACS ReturnUrl form field.",
                    ))?;
                let form_fields = HashMap::from([
                    ("ThreeDsRequest".to_string(), three_ds_request),
                    ("ReturnUrl".to_string(), return_url.to_string()),
                    ("transactionId".to_string(), operation.operation_id.clone()),
                ]);
                Some(Box::new(
                    domain_types::router_response_types::RedirectForm::Form {
                        endpoint: auth_url.clone(),
                        method: common_utils::request::Method::Post,
                        form_fields,
                    },
                ))
            }
            None => None,
        };

        // Hand back the `contractId` we created as the stable mandate
        // reference id. Nexi echoes it as `operation.order_id` (it equals the
        // `contractId` sent on `recurrence.contractId`), and this is the exact
        // value a future RepeatPayment / MIT call must replay. This matches
        // hyperswitch, which surfaces the same `connector_mandate_request_reference_id`
        // it sent — NOT the one-shot `operationId` (that is kept on
        // connector_metadata / preprocessing_id below for the 3DS continuation).
        let mandate_reference = Some(Box::new(MandateReference {
            connector_mandate_id: Some(operation.order_id.clone()),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
            mandate_metadata: None,
        }));

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // resource_id == order id (the contract reference), matching
                // hyperswitch's SetupMandate response. The Nexi `operationId`
                // needed to drive PSync / the 3DS continuation is carried on
                // `connector_feature_data` / `preprocessing_id` below, not here.
                resource_id: ResponseId::ConnectorTransactionId(operation.order_id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: Some(connector_metadata.clone()),
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(operation.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: Some(Secret::new(connector_metadata)),
                preprocessing_id: Some(operation.operation_id.clone()),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (MIT — Merchant Initiated Transaction) FLOW =====
//
// A stored NexiXPay contract is charged off-session by POSTing the saved
// `contractId` to `/orders/mit` — a single direct call, no 3DS. This mirrors
// hyperswitch, whose Authorize flow routes off-session (mandate) payments to
// `/orders/mit` with a `{ order, contractId, captureType }` body. The
// `contractId` is the value SetupMandate surfaced as
// `mandate_reference.connector_mandate_id` (== the setup call's order id), which
// the orchestrator hands back here via `mandate_reference`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexixpayMandatePaymentRequest {
    pub order: NexixpayOrderData,
    pub contract_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<NexixpayCaptureType>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NexixpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NexixpayMandatePaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        value: NexixpayRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &value.router_data;

        // The stored NexiXPay contractId to replay — surfaced by SetupMandate as
        // `mandate_reference.connector_mandate_id` and handed back here.
        let contract_id = match &item.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => mandate_data
                .get_connector_mandate_id()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "mandate_reference.connector_mandate_id",
                    context: nexi_ctx(
                        "Pass the connector_mandate_id returned by the Nexixpay SetupRecurring call.",
                        "Nexi POST /orders/mit requires contractId, which is the connector_mandate_id surfaced by SetupMandate.",
                    ),
                })?,
            // Nexi /orders/mit takes a contractId only; there is no scheme / network
            // transaction id field, so NTID-based MIT arms cannot be expressed.
            other => Err(error_stack::report!(IntegrationError::NotImplemented(
                format!("Mandate reference {other:?} for Nexixpay RepeatPayment"),
                nexi_ctx(
                    "Charge the stored mandate with a connector_mandate_id (Nexi contractId).",
                    "Nexi POST /orders/mit accepts only contractId; network transaction id MIT is not supported.",
                ),
            )))?,
        };

        // Billing / cardholder come from PaymentFlowData. There is no card on an
        // MIT, so the cardholder name is the billing full name (required), as for
        // every order-creating flow.
        let customer_info = build_customer_info(&item.resource_common_data)?;

        let order = NexixpayOrderData {
            order_id: get_nexi_order_id(&item.resource_common_data.connector_request_reference_id)?,
            amount: StringMinorUnitForConnector
                .convert(item.request.amount.amount, item.request.amount.currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: nexi_ctx(
                        "Send a valid minor-unit amount and ISO currency.",
                        "Nexi order.amount is a string in minor units; the MIT amount could not be converted.",
                    ),
                })?,
            currency: item.request.amount.currency,
            description: item.resource_common_data.description.clone(),
            customer_info,
        };

        // Capture intent mapped exactly as on Authorize; unsupported methods refused.
        let capture_type = Some(get_nexixpay_capture_type(item.request.capture_method)?);

        Ok(Self {
            order,
            contract_id: Secret::new(contract_id),
            capture_type,
        })
    }
}

// MIT response is the standard NexiXPay operation envelope (same shape as the
// Authorize/payment response).
pub type NexixpayRepeatPaymentResponse = NexixpayPaymentsResponse;

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<NexixpayRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NexixpayRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let operation = &item.response.operation;
        let status = get_payment_attempt_status(
            operation.operation_result.clone(),
            item.router_data.resource_common_data.status,
        );

        // A 2xx whose operationResult is a failure (DECLINED, DENIED_BY_RISK, FAILED, ...)
        // is an error response, as on Authorize.
        if status == AttemptStatus::Failure {
            return Ok(Self {
                response: Err(nexixpay_failure_response(
                    operation,
                    item.http_code,
                    AttemptStatus::Failure,
                )),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Typed merge of connector_feature_data: the MIT operation is the
        // authorization operation PSync reads, and an auto-captured MIT (EXECUTED)
        // records it as the capture operation a later Refund addresses.
        let connector_metadata = Some(update_nexi_meta_data(
            item.router_data
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|meta| meta.peek().clone()),
            UpdateNexixpayConnectorMetaData {
                three_d_s_auth_result: None,
                authorization_operation_id: Some(operation.operation_id.clone()),
                capture_operation_id: None,
                cancel_operation_id: None,
                psync_flow: Some(NexixpayPaymentIntent::Authorize),
                is_auto_capture: operation.operation_result == NexixpayPaymentStatus::Executed,
            },
        )?);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                // resource_id == order id, matching hyperswitch's MIT
                // (MandateResponse) mapping. The Nexi operationId used by PSync
                // is carried on connector_feature_data / authorizationOperationId.
                resource_id: ResponseId::ConnectorTransactionId(operation.order_id.clone()),
                redirection_data: None,
                // The mandate already exists; nothing new to surface here.
                mandate_reference: None,
                connector_metadata: connector_metadata.clone(),
                // Nexi defines no network transaction id and has no NTID MIT (Hyperswitch
                // sets None on every leg).
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(operation.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: connector_metadata.map(Secret::new),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== INCOMING WEBHOOK (XPay notification) =====

/// Nexi `operation.operationType` (spec: ## Status Mappings, operationType).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NexixpayOperationType {
    Authorization,
    Capture,
    Void,
    Refund,
    Cancel,
    CardVerification,
    /// An operationType this connector does not map (NOSHOW, INCREMENTAL, ...).
    #[serde(other)]
    Unknown,
}

/// Wire shape of the notification. The fields Nexi documents as required are optional here
/// only so a missing one is refused by name (`WebhookMissingRequiredField`) in
/// [`parse_nexixpay_webhook`] rather than as an anonymous decoding failure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NexixpayWebhookPayload {
    event_id: Option<String>,
    event_time: Option<String>,
    security_token: Option<Secret<String>>,
    operation: Option<NexixpayWebhookOperationPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NexixpayWebhookOperationPayload {
    order_id: Option<String>,
    operation_id: Option<String>,
    operation_type: Option<NexixpayOperationType>,
    operation_result: Option<NexixpayPaymentStatus>,
    operation_amount: Option<StringMinorUnit>,
    operation_currency: Option<common_enums::Currency>,
    additional_data: Option<NexixpayAdditionalData>,
}

/// A validated XPay notification (spec: ## Webhook Events, Webhook Payload Structure).
#[derive(Debug, Clone)]
pub struct NexixpayWebhookBody {
    pub event_id: String,
    pub event_time: String,
    pub security_token: Secret<String>,
    pub operation: NexixpayWebhookOperation,
}

/// The notification's `operation` object (same shape as `GET /operations/{operationId}`).
#[derive(Debug, Clone)]
pub struct NexixpayWebhookOperation {
    pub order_id: String,
    pub operation_id: String,
    pub operation_type: NexixpayOperationType,
    pub operation_result: NexixpayPaymentStatus,
    pub operation_amount: Option<StringMinorUnit>,
    pub operation_currency: Option<common_enums::Currency>,
    pub additional_data: Option<NexixpayAdditionalData>,
}

fn required_webhook_field(
    value: Option<String>,
    field: &'static str,
) -> Result<String, error_stack::Report<WebhookError>> {
    value
        .filter(|v| !v.is_empty())
        .ok_or_else(|| WebhookError::WebhookMissingRequiredField { field }.into())
}

/// Parses an XPay notification and applies the documented required-field check:
/// "securityToken, eventId, operationId, orderId, eventTime" must be present before the
/// notification is processed (spec: ## Webhook Events, Webhook Authentication).
pub fn parse_nexixpay_webhook(
    body: &[u8],
) -> Result<NexixpayWebhookBody, error_stack::Report<WebhookError>> {
    let payload: NexixpayWebhookPayload =
        serde_json::from_slice(body).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let security_token = payload
        .security_token
        .filter(|token| !token.peek().is_empty())
        .ok_or(WebhookError::WebhookMissingRequiredField {
            field: "securityToken",
        })?;
    let event_id = required_webhook_field(payload.event_id, "eventId")?;
    let event_time = required_webhook_field(payload.event_time, "eventTime")?;
    let operation = payload
        .operation
        .ok_or(WebhookError::WebhookMissingRequiredField { field: "operation" })?;
    let order_id = required_webhook_field(operation.order_id, "operation.orderId")?;
    let operation_id = required_webhook_field(operation.operation_id, "operation.operationId")?;
    let operation_type =
        operation
            .operation_type
            .ok_or(WebhookError::WebhookMissingRequiredField {
                field: "operation.operationType",
            })?;
    let operation_result =
        operation
            .operation_result
            .ok_or(WebhookError::WebhookMissingRequiredField {
                field: "operation.operationResult",
            })?;
    Ok(NexixpayWebhookBody {
        event_id,
        event_time,
        security_token,
        operation: NexixpayWebhookOperation {
            order_id,
            operation_id,
            operation_type,
            operation_result,
            operation_amount: operation.operation_amount,
            operation_currency: operation.operation_currency,
            additional_data: operation.additional_data,
        },
    })
}

/// Maps a notification onto its UCS event. Nexi has no event-type field: the event is
/// `operationType` x `operationResult` (spec: ## Webhook Events, Event Types). `None` means
/// the pair is not a supported event.
/// Attempt status of a notification.
///
/// Identical to [`get_payment_attempt_status`] except for `PENDING`. PSync may be answering an
/// authorization that is still waiting on the cardholder, and it can hand back the ACS
/// redirect, so `AuthenticationPending` is right there. A notification carries no redirect, and
/// on a CAPTURE it describes a payment that is already authorized -- reporting
/// `AuthenticationPending` would send it back to "needs authentication" and contradict the
/// `PaymentIntentProcessing` event [`get_nexixpay_webhook_event_type`] derives from the very
/// same body.
pub fn get_webhook_attempt_status(
    operation: &NexixpayWebhookOperation,
    prev_status: AttemptStatus,
) -> AttemptStatus {
    match (&operation.operation_type, &operation.operation_result) {
        (
            NexixpayOperationType::Authorization | NexixpayOperationType::Capture,
            NexixpayPaymentStatus::Pending,
        ) => AttemptStatus::Pending,
        (_, status) => get_payment_attempt_status(status.clone(), prev_status),
    }
}

pub fn get_nexixpay_webhook_event_type(operation: &NexixpayWebhookOperation) -> Option<EventType> {
    use NexixpayPaymentStatus as R;
    match operation.operation_type {
        NexixpayOperationType::Authorization => match operation.operation_result {
            // spec: Event Types (AUTHORIZATION / AUTHORIZED "Payment authorized")
            R::Authorized => Some(EventType::PaymentIntentAuthorizationSuccess),
            // spec: Status Mappings (EXECUTED "Payment confirmed" -> Charged)
            R::Executed => Some(EventType::PaymentIntentSuccess),
            // spec: Event Types (AUTHORIZATION / PENDING "Payment ongoing")
            R::Pending => Some(EventType::PaymentIntentProcessing),
            // spec: Event Types (DECLINED, DENIED_BY_RISK, THREEDS_FAILED, FAILED -> failure)
            R::Declined | R::DeniedByRisk | R::ThreedsFailed | R::Failed => {
                Some(EventType::PaymentIntentFailure)
            }
            // spec: Event Types (AUTHORIZATION / CANCELED "Canceled by the cardholder")
            R::Canceled => Some(EventType::PaymentIntentCancelled),
            // spec: Event Types ((any) / THREEDS_VALIDATED not mapped); others undocumented
            R::ThreedsValidated | R::Voided | R::Refunded | R::Unknown => None,
        },
        NexixpayOperationType::Capture => match operation.operation_result {
            // spec: Event Types (CAPTURE / EXECUTED "Payment confirmed")
            R::Executed => Some(EventType::PaymentIntentSuccess),
            // spec: Event Types (CAPTURE / PENDING "Payment ongoing")
            R::Pending => Some(EventType::PaymentIntentProcessing),
            // spec: Event Types (DECLINED, DENIED_BY_RISK, THREEDS_FAILED, FAILED -> failure)
            R::Declined | R::DeniedByRisk | R::ThreedsFailed | R::Failed => {
                Some(EventType::PaymentIntentFailure)
            }
            // spec: Event Types ((any) / THREEDS_VALIDATED not mapped); others undocumented
            R::Authorized
            | R::ThreedsValidated
            | R::Canceled
            | R::Voided
            | R::Refunded
            | R::Unknown => None,
        },
        NexixpayOperationType::Void => match operation.operation_result {
            // spec: Event Types (VOID / VOIDED "Online reversal of the full authorized amount")
            R::Voided => Some(EventType::PaymentIntentCancelled),
            // spec: Event Types (no other VOID result is documented)
            R::Authorized
            | R::Executed
            | R::Declined
            | R::DeniedByRisk
            | R::ThreedsValidated
            | R::ThreedsFailed
            | R::Pending
            | R::Canceled
            | R::Refunded
            | R::Failed
            | R::Unknown => None,
        },
        NexixpayOperationType::Refund => match operation.operation_result {
            // spec: Event Types (REFUND / EXECUTED, VOIDED, REFUNDED -> refund success)
            R::Executed | R::Voided | R::Refunded => Some(EventType::RefundSuccess),
            // spec: Event Types (REFUND / FAILED, DECLINED -> refund failure)
            R::Failed | R::Declined => Some(EventType::RefundFailure),
            // spec: Event Types (REFUND / PENDING -> refund processing)
            R::Pending => Some(EventType::RefundProcessing),
            // spec: Event Types (no other REFUND result is documented)
            R::Authorized
            | R::DeniedByRisk
            | R::ThreedsValidated
            | R::ThreedsFailed
            | R::Canceled
            | R::Unknown => None,
        },
        // spec: Event Types (CANCEL / any "rollback of a capture, refund" -> not mapped);
        // CARD_VERIFICATION and unknown operation types move no money and are not mapped.
        NexixpayOperationType::Cancel
        | NexixpayOperationType::CardVerification
        | NexixpayOperationType::Unknown => None,
    }
}

/// Refund status carried by a REFUND notification (spec: ## Webhook Events, Event Types).
/// `None` for a result that is not a documented refund outcome.
pub fn get_nexixpay_webhook_refund_status(result: &NexixpayPaymentStatus) -> Option<RefundStatus> {
    match result {
        // spec: Event Types (REFUND / EXECUTED, VOIDED, REFUNDED -> success)
        NexixpayPaymentStatus::Executed
        | NexixpayPaymentStatus::Voided
        | NexixpayPaymentStatus::Refunded => Some(RefundStatus::Success),
        // spec: Event Types (REFUND / PENDING -> processing)
        NexixpayPaymentStatus::Pending => Some(RefundStatus::Pending),
        // spec: Event Types (REFUND / FAILED, DECLINED -> failure)
        NexixpayPaymentStatus::Failed | NexixpayPaymentStatus::Declined => {
            Some(RefundStatus::Failure)
        }
        // spec: Event Types (no other REFUND result is documented)
        NexixpayPaymentStatus::Authorized
        | NexixpayPaymentStatus::DeniedByRisk
        | NexixpayPaymentStatus::ThreedsValidated
        | NexixpayPaymentStatus::ThreedsFailed
        | NexixpayPaymentStatus::Canceled
        | NexixpayPaymentStatus::Unknown => None,
    }
}
