use common_enums::{AttemptStatus, RechargeStatus, RefundStatus};
use common_utils::types::FloatMajorUnit;
use domain_types::{
    connector_flow::{
        Authorize, CreatePaymentMethod, GetPaymentMethod, PaymentMethodEligibility, Recharge,
        Refund, ServerAuthenticationToken,
    },
    connector_types::{
        CreatePaymentMethodData, CreatePaymentMethodResponseData, CustomerInfo,
        GetPaymentMethodData, GetPaymentMethodResponseData, PaymentFlowData,
        PaymentMethodEligibilityData, PaymentMethodEligibilityResponse, PaymentsAuthorizeData,
        PaymentsResponseData, RawConnectorStatus, RechargeRequestData, RechargeResponseData,
        RefundFlowData, RefundsData, RefundsResponseData, ResponseId,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{self, PaymentMethodDataTypes, PaymentMethodDetails, WalletDetails},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::qwikcilver::QwikcilverRouterData, types::ResponseRouterData};

pub(crate) const QWIKCILVER_INTEGRATION_DOC_URL: &str = "https://developers.qwikcilver.com/";

pub(crate) const TOKEN_TYPE_BEARER: &str = "Bearer";

pub(crate) mod qwikcilver_status {
    pub(crate) const ACTIVE: &str = "ACTIVE";
    pub(crate) const INACTIVE: &str = "INACTIVE";
    pub(crate) const DEPLETED: &str = "DEPLETED";
    pub(crate) const EXPIRED: &str = "EXPIRED";
}

pub(crate) fn qc_err_ctx(
    additional_context: impl Into<String>,
    suggested_action: impl Into<String>,
) -> domain_types::errors::IntegrationErrorContext {
    domain_types::errors::IntegrationErrorContext {
        additional_context: Some(additional_context.into()),
        suggested_action: Some(suggested_action.into()),
        doc_url: Some(QWIKCILVER_INTEGRATION_DOC_URL.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct QwikcilverAuthType {
    pub(super) bootstrap_bearer_token: Secret<String>,
    pub(super) terminal_id: Secret<String>,
    pub(super) username: Secret<String>,
    pub(super) password: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for QwikcilverAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(value: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match value {
            ConnectorSpecificConfig::Qwikcilver {
                bootstrap_bearer_token,
                terminal_id,
                username,
                password,
                ..
            } => Ok(Self {
                bootstrap_bearer_token: bootstrap_bearer_token.clone(),
                terminal_id: terminal_id.clone(),
                username: username.clone(),
                password: password.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: qc_err_ctx(
                    "x-connector-config did not deserialize as the `Qwikcilver` variant — \
                     the resolved variant doesn't carry the required Pine Labs fields.",
                    "Send `x-connector-config: {\"config\":{\"Qwikcilver\":{\"bootstrap_bearer_token\":\"…\",\
                     \"terminal_id\":\"…\",\"username\":\"…\",\"password\":\"…\"}}}` exactly. \
                     Header value parsing is case-sensitive; the variant key must be `Qwikcilver`.",
                ),
            }
            .into()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverAuthorizeRequest {
    #[serde(rename = "TerminalID")]
    pub terminal_id: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub transaction_id: u64,
    pub date_at_client: String,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for QwikcilverAuthorizeRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = QwikcilverAuthType::try_from(&item.router_data.connector_config)?;
        let common = &item.router_data.resource_common_data;
        let transaction_id =
            derive_transaction_id_from_reference(&common.connector_request_reference_id);
        let date_at_client = current_datetime_qwikcilver();
        Ok(Self {
            terminal_id: auth.terminal_id,
            username: auth.username,
            password: auth.password,
            transaction_id,
            date_at_client,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QwikcilverAuthorizeResponse {
    pub auth_token: Secret<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub batch_id: Option<i64>,
}

const SESSION_EXPIRY_SECONDS: i64 = 60 * 20;

impl<F> TryFrom<ResponseRouterData<QwikcilverAuthorizeResponse, Self>>
    for RouterDataV2<
        F,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => Ok(ServerAuthenticationTokenResponseData {
                access_token: body.auth_token,
                token_type: Some(TOKEN_TYPE_BEARER.to_string()),
                expires_in: Some(SESSION_EXPIRY_SECONDS),
            }),
            // Bootstrap errors aren't payment attempts → no AttemptStatus.
            _ => Err(error_response_from_qc(
                (&body).into(),
                None,
                item.http_code,
                None,
            )),
        };
        Ok(data)
    }
}

pub(crate) const QWIKCILVER_SUCCESS_CODE: i64 = 0;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWallet {
    pub wallet_number: Secret<String>,
    pub external_wallet_id: Option<Secret<String>>,
    pub wallet_pin: Option<Secret<String>>,
    pub status: Option<String>,
    pub wallet_program_group_name: Option<String>,
    pub wallet_holder_name: Option<Secret<String>>,
    pub balance: FloatMajorUnit,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub customer: Option<QwikcilverCustomer>,
    pub card: Option<QwikcilverCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCustomer {
    pub customer_type: Option<String>,
    pub salutation: Option<String>,
    // Pine Labs wire is inconsistent: `Firstname` (one word) vs `LastName` (two); only `first_name` needs an explicit rename.
    #[serde(rename = "Firstname")]
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone_number: Option<Secret<String>>,
    // Pine Labs occasionally stuffs a phone number into Email; lenient deserializer drops
    // anything that isn't a valid RFC-5322 address rather than failing the whole response.
    #[serde(default, deserialize_with = "deserialize_lenient_email")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(rename = "DOB")]
    pub dob: Option<Secret<String>>,
    pub address_line1: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub external_customer_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCard {
    pub card_number: Secret<String>,
    pub amount: FloatMajorUnit,
    pub card_program_name: Option<String>,
    pub card_status: Option<String>,
    pub card_type: Option<String>,
    pub expiry: Option<Secret<String>>,
    pub bucket_type: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRedeemRequest {
    pub idempotency_key: String,
    pub invoice_number: String,
    pub amount: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_amount: Option<FloatMajorUnit>,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for QwikcilverRedeemRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
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
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: qc_err_ctx(
                    format!(
                        "Failed to convert Redeem amount {} {} to FloatMajorUnit. \
                         Qwikcilver expects major-unit decimals (e.g. 0.20 AED).",
                        item.router_data.request.minor_amount.get_amount_as_i64(),
                        item.router_data.request.currency,
                    ),
                    "Verify `amount.minor_amount` is a non-negative integer and \
                     `amount.currency` is a 3-letter ISO 4217 code that Pine Labs supports \
                     for your terminal (e.g. AED, INR).",
                ),
            })
            .attach_printable_lazy(|| {
                format!(
                    "qwikcilver: Redeem amount conversion failed for currency={}",
                    item.router_data.request.currency
                )
            })?;
        let idempotency_key = item
            .router_data
            .resource_common_data
            .get_merchant_request_id()?;
        let invoice_number = item
            .router_data
            .request
            .merchant_order_id
            .clone()
            .ok_or_else(domain_types::utils::missing_field_err("merchant_order_id"))?;
        Ok(Self {
            idempotency_key,
            invoice_number,
            amount,
            notes: None,
            bill_amount: Some(amount),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRedeemResponse {
    pub current_batch_number: i64,
    pub wallet_number: Secret<String>,
    pub invoice_number: Option<String>,
    pub date_at_server: Option<String>,
    pub batch_number: i64,
    pub amount: FloatMajorUnit,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub bill_amount: Option<FloatMajorUnit>,
    pub excluded_buckets_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl<T> TryFrom<ResponseRouterData<QwikcilverRedeemResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverRedeemResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.resource_common_data.raw_connector_status = Some(raw_connector_status_from_qc(&body));
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => {
                data.resource_common_data.status = REDEEM_SUCCESS_STATUS;
                Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        body.transaction_id.to_string(),
                    ),
                    redirection_data: None,
                    connector_metadata: Some(serde_json::json!({
                        "batch_number": body.batch_number,
                    })),
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: body.invoice_number,
                    incremental_authorization_allowed: None,
                    mandate_reference: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                })
            }
            _ => Err(error_response_from_qc(
                (&body).into(),
                Some(body.transaction_id.to_string()),
                item.http_code,
                Some(AttemptStatus::Failure),
            )),
        };
        Ok(data)
    }
}

pub(crate) fn qwikcilver_wallet_number_from_refund(
    req: &RefundsData,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    use payment_method_data::{PaymentMethodData, WalletData};
    match req.payment_method_data.as_ref() {
        Some(PaymentMethodData::Wallet(WalletData::QwikcilverWalletDirect(d))) => {
            Ok(d.wallet_number.clone())
        }
        _ => Err(error_stack::report!(
            IntegrationError::MissingRequiredField {
                field_name: "payment_method.qwikcilver_wallet_direct.wallet_number",
                context: qc_err_ctx(
                    "Cancel Redeem needs the wallet number of the wallet whose Redeem is being \
                 reversed. Pass it on the typed `payment_method.qwikcilver_wallet_direct` \
                 variant — the same shape Authorize uses.",
                    "Send `payment_method: { payment_method: { qwikcilver_wallet_direct: \
                 { wallet_number: \"<wn>\" } } }` on the Refund request.",
                ),
            }
        )),
    }
}

pub(crate) fn qwikcilver_wallet_number_from_authorize<T>(
    req: &PaymentsAuthorizeData<T>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    use domain_types::payment_method_data::{PaymentMethodData, WalletData};
    match &req.payment_method_data {
        PaymentMethodData::Wallet(WalletData::QwikcilverWalletDirect(d)) => {
            Ok(d.wallet_number.clone())
        }
        _ => Err(error_stack::report!(IntegrationError::MissingRequiredField {
            field_name: "payment_method.qwikcilver_wallet_direct.wallet_number",
            context: qc_err_ctx(
                "Qwikcilver Redeem requires the destination wallet number via the typed \
                 `qwikcilver_wallet_direct` payment_method variant — no other PaymentMethod variant \
                 is supported on this connector.",
                "Send `payment_method: { payment_method: { qwikcilver_wallet_direct: { \
                 wallet_number: \"<wn>\" } } }` on the Authorize request.",
            ),
        })),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCancelRedeemBody {
    pub original_batch_number: i64,
    pub original_transaction_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for QwikcilverCancelRedeemBody
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let txn = req.connector_transaction_id.parse::<i64>().map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_transaction_id",
                context: qc_err_ctx(
                    format!(
                        "Cancel Redeem reads the original Redeem's `TransactionId` from \
                         `connector_transaction_id`; expected a numeric `i64`, got \
                         `{}`.",
                        req.connector_transaction_id
                    ),
                    "Pass the `connector_transaction_id` from the original Redeem response \
                     verbatim (it's just the Pine Labs txn id as a numeric string).",
                ),
            })
        })?;
        let batch = item
            .router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .map(|s| s.peek())
            .and_then(|v| v.get("batch_number"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "connector_feature_data.batch_number",
                    context: qc_err_ctx(
                        "Cancel Redeem also needs the original Redeem's batch number; supply \
                         it on `connector_feature_data.batch_number`. Capture it from the \
                         Authorize response's `connector_metadata.batch_number`.",
                        "Send `connector_feature_data` as a JSON-string containing \
                         `batch_number` (i64), e.g. `{\"batch_number\":17322479}`.",
                    ),
                })
            })?;
        Ok(Self {
            original_batch_number: batch,
            original_transaction_id: txn,
            notes: req.reason.clone(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCancelRedeemResponse {
    pub current_batch_number: i64,
    pub invoice_number: Option<String>,
    pub bill_amount: Option<FloatMajorUnit>,
    pub wallet_number: Option<Secret<String>>,
    pub batch_number: Option<i64>,
    pub amount: Option<FloatMajorUnit>,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl TryFrom<ResponseRouterData<QwikcilverCancelRedeemResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverCancelRedeemResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.resource_common_data.raw_connector_status = Some(raw_connector_status_from_qc(&body));
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => Ok(RefundsResponseData {
                connector_refund_id: body.transaction_id.to_string(),
                refund_status: CANCEL_REDEEM_SUCCESS_STATUS,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            _ => Err(error_response_from_qc(
                (&body).into(),
                Some(body.transaction_id.to_string()),
                item.http_code,
                None,
            )),
        };
        Ok(data)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRechargeRequest {
    pub idempotency_key: String,
    pub amount: FloatMajorUnit,
    pub card_program_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub invoice_number: String,
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
            T,
        >,
    > for QwikcilverRechargeRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let amount = item
            .connector
            .amount_converter
            .convert(req.amount, req.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: qc_err_ctx(
                    format!(
                        "Failed to convert Recharge amount {} {} to FloatMajorUnit.",
                        req.amount.get_amount_as_i64(),
                        req.currency,
                    ),
                    "Verify `amount.minor_amount` is a non-negative integer and \
                     `amount.currency` is supported by your Pine Labs program (e.g. AED for \
                     `Blue Retail UAE Refund eCard`).",
                ),
            })
            .attach_printable_lazy(|| {
                format!(
                    "qwikcilver: Recharge amount conversion failed for currency={}",
                    req.currency
                )
            })?;
        let invoice_number = req.merchant_recharge_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "merchant_recharge_id",
                context: qc_err_ctx(
                    "Pine Labs `InvoiceNumber` on Add Card must be unique per recharge attempt; \
                     `merchant_recharge_id` is the natural per-recharge identifier.",
                    "Set `merchant_recharge_id` on every PaymentMethodService.Recharge request, \
                     e.g. `\"rch-<merchant>-<order>-<timestamp>\"`.",
                ),
            })
        })?;
        let idempotency_key = req.merchant_request_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "merchant_request_id",
                context: qc_err_ctx(
                    "Pine Labs `IdempotencyKey` on Add Card guards against double-credit on \
                     retried submissions; `merchant_request_id` is the natural per-request token.",
                    "Set `merchant_request_id` on every PaymentMethodService.Recharge request \
                     to a value unique per logical attempt.",
                ),
            })
        })?;
        Ok(Self {
            invoice_number,
            idempotency_key,
            amount,
            card_program_name: req.product_id.clone(),
            notes: req.description.clone(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverRechargeResponse {
    pub current_batch_number: i64,
    pub invoice_number: Option<String>,
    pub bill_amount: Option<FloatMajorUnit>,
    pub wallet: Option<QwikcilverWallet>,
    pub amount: Option<FloatMajorUnit>,
    pub balance: Option<FloatMajorUnit>,
    pub consolidated_balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: i64,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

impl TryFrom<ResponseRouterData<QwikcilverRechargeResponse, Self>>
    for RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverRechargeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => {
                let connector_recharge_id = body.transaction_id.to_string();
                let merchant_recharge_id = data.request.merchant_recharge_id.clone();
                let connector_payment_method_id = body
                    .wallet
                    .as_ref()
                    .map(|w| w.wallet_number.clone().expose())
                    .or_else(|| data.request.connector_payment_method_id.clone());
                let recharge_currency = data.request.currency;
                let payment_method_details = body.wallet.as_ref().map(|w| {
                    let balance =
                        crate::connectors::qwikcilver::QwikcilverAmountConvertor::convert_back(
                            w.balance,
                            recharge_currency,
                        )
                        .ok();
                    let items = w
                        .card
                        .as_ref()
                        .and_then(|c| card_to_wallet_item(c, Some(recharge_currency)))
                        .map(|item| vec![item])
                        .unwrap_or_default();
                    PaymentMethodDetails::Wallet(WalletDetails {
                        wallet_account_id: w.wallet_number.clone().expose(),
                        wallet_pin: w.wallet_pin.clone(),
                        wallet_status: map_wallet_status(w.status.as_ref()),
                        wallet_holder_name: w
                            .wallet_holder_name
                            .as_ref()
                            .map(|h| h.clone().expose()),
                        balance,
                        product_id: w.wallet_program_group_name.clone(),
                        items,
                    })
                });
                Ok(RechargeResponseData {
                    merchant_payment_method_id: data.request.merchant_payment_method_id.clone(),
                    connector_payment_method_id,
                    merchant_recharge_id,
                    connector_recharge_id: Some(connector_recharge_id),
                    status: RECHARGE_SUCCESS_STATUS,
                    payment_method_details,
                    status_code: item.http_code,
                })
            }
            _ => Err(error_response_from_qc(
                (&body).into(),
                Some(body.transaction_id.to_string()),
                item.http_code,
                None,
            )),
        };
        Ok(data)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCreateWalletRequest {
    // Wire key is `Externalwalletid` (lowercase `w`), not PascalCase.
    #[serde(rename = "Externalwalletid")]
    pub external_wallet_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_program_group_name: Option<String>,
    pub customer: QwikcilverCreateCustomer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverCreateCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salutation: Option<String>,
    #[serde(rename = "Firstname")]
    pub first_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    pub phone_number: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_notification_language: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct QwikcilverCreateFeatureData {
    #[serde(default)]
    pub customer_type: Option<String>,
}

impl QwikcilverCreateFeatureData {
    pub(crate) fn from_request(
        req: &CreatePaymentMethodData,
    ) -> Result<Self, error_stack::Report<IntegrationError>> {
        let Some(raw) = req.connector_feature_data.as_ref() else {
            return Ok(Self::default());
        };
        serde_json::from_value(raw.clone().expose()).map_err(|e| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "connector_feature_data",
                context: qc_err_ctx(
                    format!("invalid Qwikcilver connector_feature_data for Create: {e}"),
                    "Send `connector_feature_data` as a JSON-string (e.g. \
                     `\"{\\\"currency\\\":\\\"AED\\\",\\\"customer_type\\\":\\\"INDIVIDUAL\\\"}\"`) \
                     — not a nested JSON object. The string is parsed server-side after \
                     secret-unwrap.",
                ),
            })
        })
    }
}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
            T,
        >,
    > for QwikcilverCreateWalletRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: QwikcilverRouterData<
            RouterDataV2<
                CreatePaymentMethod,
                PaymentFlowData,
                CreatePaymentMethodData,
                CreatePaymentMethodResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let req = &item.router_data.request;
        let customer = req.customer.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "customer",
                context: qc_err_ctx(
                    "Qwikcilver provisions wallets against a real customer record — the \
                     `customer` block on PaymentMethodService.Create is required, not optional.",
                    "Send a full `customer` object including `first_name` and `phone_number` \
                     at minimum. `phone_number` becomes the wallet's `ExternalWalletId`.",
                ),
            })
        })?;
        let phone = customer.get_phone_number()?;
        let first = customer.get_first_name()?;
        let feature = QwikcilverCreateFeatureData::from_request(req)?;
        let program = req.product_id.clone();
        Ok(Self {
            external_wallet_id: phone.clone(),
            wallet_program_group_name: program,
            customer: QwikcilverCreateCustomer {
                customer_type: feature.customer_type,
                salutation: customer.salutation.clone(),
                first_name: first,
                last_name: customer.last_name.clone(),
                phone_number: phone,
                email: customer.customer_email.clone(),
                preferred_notification_language: None,
            },
            notes: req.description.clone(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct QwikcilverEmptyBody {}

impl<T>
    TryFrom<
        QwikcilverRouterData<
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
            T,
        >,
    > for QwikcilverEmptyBody
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: QwikcilverRouterData<
            RouterDataV2<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWalletEnvelope {
    // Pine Labs's by-external-id lookup returns the same `Wallet` envelope on not-found, but
    // with every field null (including WalletNumber). Custom deserializer drops the block to
    // `None` in that case so the strict WalletDetails contract holds for all success responses.
    #[serde(default, deserialize_with = "deserialize_wallet_or_null_fields")]
    pub wallet: Option<QwikcilverWalletDetails>,
    pub current_batch_number: Option<i64>,
    pub notes: Option<String>,
    pub approval_code: Option<String>,
    pub response_code: i64,
    pub response_message: Option<String>,
    pub transaction_id: Option<i64>,
    pub transaction_type: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
}

fn deserialize_lenient_email<'de, D>(
    deserializer: D,
) -> Result<Option<common_utils::pii::Email>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = Option::deserialize(deserializer)?;
    Ok(raw.and_then(|s| {
        if s.is_empty() {
            return None;
        }
        serde_json::from_value::<common_utils::pii::Email>(serde_json::Value::String(s)).ok()
    }))
}

fn deserialize_wallet_or_null_fields<'de, D>(
    deserializer: D,
) -> Result<Option<QwikcilverWalletDetails>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    let Some(value) = raw else {
        return Ok(None);
    };
    if value.is_null()
        || value
            .get("WalletNumber")
            .is_none_or(serde_json::Value::is_null)
    {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct QwikcilverGetWalletResponse(pub QwikcilverWalletEnvelope);

/// Distinct response newtype for `PaymentMethodEligibility`. Wraps the identical
/// `QwikcilverWalletEnvelope` payload `GetPaymentMethod` parses — macro-generated templating
/// types are keyed by response type name, so this flow needs its own type to avoid colliding
/// with `GetPaymentMethod`'s templating impl, even though it's the same connector call.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct QwikcilverEligibilityResponse(pub QwikcilverWalletEnvelope);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverWalletDetails {
    pub wallet_number: Secret<String>,
    pub external_wallet_id: Option<Secret<String>>,
    pub wallet_pin: Option<Secret<String>>,
    pub status: Option<String>,
    pub track_data: Option<Secret<String>>,
    pub bar_code: Option<Secret<String>>,
    pub wallet_program_group_name: Option<String>,
    pub wallet_holder_name: Option<Secret<String>>,
    pub balance: Option<FloatMajorUnit>,
    pub notes: Option<String>,
    pub card: Option<QwikcilverCard>,
    pub customer: Option<QwikcilverCustomer>,
}

const REDEEM_SUCCESS_STATUS: AttemptStatus = AttemptStatus::Charged;
const CANCEL_REDEEM_SUCCESS_STATUS: RefundStatus = RefundStatus::Success;
const RECHARGE_SUCCESS_STATUS: RechargeStatus = RechargeStatus::Success;

fn map_wallet_status(s: Option<&String>) -> Option<common_enums::WalletStatus> {
    s.and_then(|raw| match raw.to_ascii_uppercase().as_str() {
        qwikcilver_status::ACTIVE => Some(common_enums::WalletStatus::Active),
        qwikcilver_status::INACTIVE => Some(common_enums::WalletStatus::Inactive),
        _ => None,
    })
}

fn map_wallet_item_status(s: Option<&String>) -> common_enums::WalletItemStatus {
    s.map(|raw| match raw.to_ascii_uppercase().as_str() {
        qwikcilver_status::ACTIVE => common_enums::WalletItemStatus::Active,
        qwikcilver_status::INACTIVE => common_enums::WalletItemStatus::Inactive,
        qwikcilver_status::DEPLETED => common_enums::WalletItemStatus::Depleted,
        qwikcilver_status::EXPIRED => common_enums::WalletItemStatus::Expired,
        _ => common_enums::WalletItemStatus::Unspecified,
    })
    .unwrap_or_default()
}

const LAST_FOUR_DIGITS_LEN: usize = 4;

fn card_to_wallet_item(
    card: &QwikcilverCard,
    currency: Option<common_enums::Currency>,
) -> Option<payment_method_data::WalletItem> {
    let pan = card.card_number.peek();
    // char-safe last-4 — byte-slicing would panic on a non-ASCII byte at the boundary.
    let mut last_four: Vec<char> = pan.chars().rev().take(LAST_FOUR_DIGITS_LEN).collect();
    if last_four.len() < LAST_FOUR_DIGITS_LEN {
        return None;
    }
    last_four.reverse();
    let wallet_item_id: String = last_four.into_iter().collect();
    let available_balance = currency.and_then(|c| {
        crate::connectors::qwikcilver::QwikcilverAmountConvertor::convert_back(card.amount, c)
            .ok()
            .map(|minor| common_utils::types::Money {
                amount: minor,
                currency: c,
            })
    });
    Some(payment_method_data::WalletItem {
        wallet_item_id,
        product_id: card.card_program_name.clone().unwrap_or_default(),
        status: map_wallet_item_status(card.card_status.as_ref()),
        available_balance,
        expiry_date: card.expiry.as_ref().map(|e| e.peek().to_string()),
    })
}

fn wallet_details_to_payment_method_details(
    wallet: &QwikcilverWalletDetails,
    currency: Option<common_enums::Currency>,
) -> PaymentMethodDetails {
    let balance = wallet.balance.zip(currency).and_then(|(b, c)| {
        crate::connectors::qwikcilver::QwikcilverAmountConvertor::convert_back(b, c).ok()
    });
    let items = wallet
        .card
        .as_ref()
        .and_then(|c| card_to_wallet_item(c, currency))
        .map(|item| vec![item])
        .unwrap_or_default();
    PaymentMethodDetails::Wallet(WalletDetails {
        wallet_account_id: wallet.wallet_number.clone().expose(),
        wallet_pin: wallet.wallet_pin.clone(),
        wallet_status: map_wallet_status(wallet.status.as_ref()),
        wallet_holder_name: wallet
            .wallet_holder_name
            .as_ref()
            .map(|h| h.clone().expose()),
        balance,
        product_id: wallet.wallet_program_group_name.clone(),
        items,
    })
}

fn currency_from_feature_data(
    feature: Option<&common_utils::pii::SecretSerdeValue>,
) -> Option<common_enums::Currency> {
    feature
        .map(|s| s.peek())
        .and_then(|v| v.get("currency"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
}

fn customer_details_to_customer_info(
    customer: &QwikcilverCustomer,
    wallet_external_id: Option<&Secret<String>>,
) -> CustomerInfo {
    // Pine Labs convention: ExternalWalletId IS the customer's mobile.
    let customer_phone_number = customer
        .phone_number
        .clone()
        .or_else(|| wallet_external_id.cloned());
    let customer_id = customer.external_customer_id.as_ref().and_then(|id| {
        common_utils::id_type::CustomerId::try_from(std::borrow::Cow::from(id.clone().expose()))
            .ok()
    });
    CustomerInfo {
        customer_id,
        customer_email: customer.email.clone(),
        customer_name: None,
        first_name: customer.first_name.clone(),
        last_name: customer.last_name.clone(),
        customer_phone_number,
        customer_phone_country_code: None,
        salutation: customer.salutation.clone(),
        date_of_birth: None,
    }
}

impl TryFrom<ResponseRouterData<QwikcilverWalletEnvelope, Self>>
    for RouterDataV2<
        CreatePaymentMethod,
        PaymentFlowData,
        CreatePaymentMethodData,
        CreatePaymentMethodResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverWalletEnvelope, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = match (body.response_code, body.wallet.as_ref()) {
            (QWIKCILVER_SUCCESS_CODE, Some(wallet)) => {
                let merchant_pm_id = data.request.merchant_payment_method_id.clone();
                let currency =
                    currency_from_feature_data(data.request.connector_feature_data.as_ref());
                Ok(CreatePaymentMethodResponseData {
                    merchant_payment_method_id: merchant_pm_id,
                    connector_payment_method_id: Some(wallet.wallet_number.clone().expose()),
                    payment_method_details: Some(wallet_details_to_payment_method_details(
                        wallet, currency,
                    )),
                    customer: wallet.customer.as_ref().map(|c| {
                        customer_details_to_customer_info(c, wallet.external_wallet_id.as_ref())
                    }),
                    status_code: item.http_code,
                })
            }
            // Violates PDF §10.2.2 — every successful Create must return a Wallet.
            (QWIKCILVER_SUCCESS_CODE, None) => Err(error_response_from_qc(
                QwikcilverErrorResponse {
                    response_code: Some(QWIKCILVER_SUCCESS_CODE),
                    response_message: Some(
                        "Qwikcilver Create returned success but the Wallet block was missing \
                         from the response — Pine Labs's contract requires a Wallet snapshot \
                         on every successful provisioning."
                            .to_string(),
                    ),
                    error_code: None,
                    error_description: None,
                    wallet_number: None,
                },
                body.transaction_id.map(|t| t.to_string()),
                item.http_code,
                None,
            )),
            _ => {
                let txn_id = body.transaction_id.map(|t| t.to_string());
                Err(error_response_from_qc(
                    (&body).into(),
                    txn_id,
                    item.http_code,
                    None,
                ))
            }
        };
        Ok(data)
    }
}

impl TryFrom<ResponseRouterData<QwikcilverGetWalletResponse, Self>>
    for RouterDataV2<
        GetPaymentMethod,
        PaymentFlowData,
        GetPaymentMethodData,
        GetPaymentMethodResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverGetWalletResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response.0;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => {
                let merchant_pm_id = data.request.merchant_payment_method_id.clone();
                let currency =
                    currency_from_feature_data(data.request.connector_feature_data.as_ref());
                let (connector_pm_id, payment_method_details, customer) = if let Some(wallet) =
                    body.wallet.as_ref()
                {
                    (
                        Some(wallet.wallet_number.clone().expose()),
                        Some(wallet_details_to_payment_method_details(wallet, currency)),
                        wallet.customer.as_ref().map(|c| {
                            customer_details_to_customer_info(c, wallet.external_wallet_id.as_ref())
                        }),
                    )
                } else {
                    (data.request.connector_payment_method_id.clone(), None, None)
                };
                Ok(GetPaymentMethodResponseData {
                    merchant_payment_method_id: merchant_pm_id,
                    connector_payment_method_id: connector_pm_id,
                    payment_method_details,
                    customer,
                    status_code: item.http_code,
                })
            }
            _ => {
                let txn_id = body.transaction_id.map(|t| t.to_string());
                Err(error_response_from_qc(
                    (&body).into(),
                    txn_id,
                    item.http_code,
                    None,
                ))
            }
        };
        Ok(data)
    }
}

/// Performs the exact same wallet lookup as `GetPaymentMethod` and derives eligibility from
/// the wallet's status: ACTIVE → Eligible, INACTIVE → Ineligible. The resolved wallet's
/// payment method details (balance, items, etc.) are returned alongside the eligibility
/// verdict in the same response.
impl TryFrom<ResponseRouterData<QwikcilverEligibilityResponse, Self>>
    for RouterDataV2<
        PaymentMethodEligibility,
        PaymentFlowData,
        PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<QwikcilverEligibilityResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let body = item.response.0;
        data.resource_common_data.raw_connector_response =
            serde_json::to_string(&body).ok().map(Secret::new);
        data.response = match body.response_code {
            QWIKCILVER_SUCCESS_CODE => {
                let currency = data.request.amount.currency;
                let (eligibility, payment_method_details) =
                    if let Some(wallet) = body.wallet.as_ref() {
                        let eligibility = match map_wallet_status(wallet.status.as_ref()) {
                            Some(common_enums::WalletStatus::Active) => {
                                common_enums::EligibilityStatus::Eligible
                            }
                            Some(common_enums::WalletStatus::Inactive) => {
                                common_enums::EligibilityStatus::Ineligible
                            }
                            Some(common_enums::WalletStatus::Unspecified) | None => {
                                common_enums::EligibilityStatus::Unknown
                            }
                        };
                        (
                            eligibility,
                            Some(wallet_details_to_payment_method_details(
                                wallet,
                                Some(currency),
                            )),
                        )
                    } else {
                        (common_enums::EligibilityStatus::Unknown, None)
                    };
                Ok(PaymentMethodEligibilityResponse {
                    eligibility,
                    payment_method_details,
                    status_code: u32::from(item.http_code),
                })
            }
            _ => {
                let txn_id = body.transaction_id.map(|t| t.to_string());
                Err(error_response_from_qc(
                    (&body).into(),
                    txn_id,
                    item.http_code,
                    None,
                ))
            }
        };
        Ok(data)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QwikcilverErrorResponse {
    pub response_code: Option<i64>,
    pub response_message: Option<String>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,

    pub wallet_number: Option<Secret<String>>,
}

impl From<&QwikcilverAuthorizeResponse> for QwikcilverErrorResponse {
    fn from(r: &QwikcilverAuthorizeResponse) -> Self {
        Self {
            response_code: Some(r.response_code),
            response_message: r.response_message.clone(),
            error_code: None,
            error_description: None,
            wallet_number: None,
        }
    }
}

impl From<&QwikcilverRedeemResponse> for QwikcilverErrorResponse {
    fn from(r: &QwikcilverRedeemResponse) -> Self {
        Self {
            response_code: Some(r.response_code),
            response_message: r.response_message.clone(),
            error_code: r.error_code.clone(),
            error_description: r.error_description.clone(),
            wallet_number: Some(r.wallet_number.clone()),
        }
    }
}

impl From<&QwikcilverCancelRedeemResponse> for QwikcilverErrorResponse {
    fn from(r: &QwikcilverCancelRedeemResponse) -> Self {
        Self {
            response_code: Some(r.response_code),
            response_message: r.response_message.clone(),
            error_code: r.error_code.clone(),
            error_description: r.error_description.clone(),
            wallet_number: r.wallet_number.clone(),
        }
    }
}

impl From<&QwikcilverRechargeResponse> for QwikcilverErrorResponse {
    fn from(r: &QwikcilverRechargeResponse) -> Self {
        Self {
            response_code: Some(r.response_code),
            response_message: r.response_message.clone(),
            error_code: r.error_code.clone(),
            error_description: r.error_description.clone(),
            wallet_number: r.wallet.as_ref().map(|w| w.wallet_number.clone()),
        }
    }
}

impl From<&QwikcilverWalletEnvelope> for QwikcilverErrorResponse {
    fn from(r: &QwikcilverWalletEnvelope) -> Self {
        Self {
            response_code: Some(r.response_code),
            response_message: r.response_message.clone(),
            error_code: r.error_code.clone(),
            error_description: r.error_description.clone(),
            wallet_number: r.wallet.as_ref().map(|w| w.wallet_number.clone()),
        }
    }
}

/// Per QwikWallet RestAPI V2 §6 `APIResponse`, every Qwikcilver endpoint reports its outcome
/// with the same quad, on success as well as on failure:
///
/// - `ResponseCode`     — "Generated by Server. Value 0 indicates success, rest is failure."
///   This is the Pine Labs status code enumerated in §4 Common Response Codes (0, 10010
///   "Balance is insufficient", 10042, …), so it is the raw status code.
/// - `ResponseMessage`  — the accompanying status text ("Transaction successful.", …).
/// - `ErrorCode`        — "Actual error/exception details returned in case of system error".
///   Despite the name it is *not* the business status code — it is only populated on system
///   exceptions (the 50x class), so it must not shadow `ResponseCode`.
/// - `ErrorDescription` — "Description of the error code", the companion to `ErrorCode`.
///
/// `QwikcilverErrorResponse` already normalises that quad for each response shape, so this
/// reuses those `From` impls as the single source for the raw status.
///
/// Only Authorize (Redeem) and Refund (Cancel Redeem) call this: they are the flows whose gRPC
/// response — `PaymentServiceAuthorizeResponse` and `RefundResponse` — carries a
/// `raw_connector_status` field. The PaymentMethodService responses (Recharge, Create, Get,
/// Eligibility) have no such field, so setting it on their flow data would be discarded.
pub(crate) fn raw_connector_status_from_qc<'a, R>(response: &'a R) -> RawConnectorStatus
where
    QwikcilverErrorResponse: From<&'a R>,
{
    let status = QwikcilverErrorResponse::from(response);
    RawConnectorStatus {
        code: status.response_code.map(|code| code.to_string()),
        message: status.response_message,
        // System-error detail, when Qwikcilver sends any — surfaced as the supporting reason
        // rather than as the code.
        reason: match (status.error_code, status.error_description) {
            (Some(code), Some(description)) => Some(format!("{code}: {description}")),
            (Some(detail), None) | (None, Some(detail)) => Some(detail),
            (None, None) => None,
        },
    }
}

pub(crate) fn error_response_from_qc(
    err: QwikcilverErrorResponse,
    connector_txn_id: Option<String>,
    http_code: u16,
    attempt_status: Option<AttemptStatus>,
) -> ErrorResponse {
    make_error_response(
        err.response_code.unwrap_or(0),
        err.response_message,
        err.error_code,
        err.error_description,
        connector_txn_id,
        http_code,
        attempt_status,
    )
}

pub(crate) fn derive_transaction_id_from_reference(reference_id: &str) -> u64 {
    if let Ok(n) = reference_id.parse::<i64>() {
        if let Ok(positive) = u64::try_from(n) {
            if positive > 0 {
                return positive;
            }
        }
    }
    if reference_id.is_empty() {
        use std::time::{SystemTime, UNIX_EPOCH};
        return SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_nanos()).ok())
            .map(|nanos| nanos & 0x7FFF_FFFF_FFFF_FFFF)
            .unwrap_or(1);
    }
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(reference_id.as_bytes());
    let mut buf = [0u8; 8];
    let (head, _) = digest.split_at(buf.len());
    buf.copy_from_slice(head);
    u64::from_be_bytes(buf) & 0x3FFF_FFFF_FFFF_FFFF
}

pub(crate) fn current_datetime_qwikcilver() -> String {
    let now = time::OffsetDateTime::now_utc();
    let (h, m, s) = (now.hour(), now.minute(), now.second());
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        h,
        m,
        s,
    )
}

fn make_error_response(
    response_code: i64,
    response_message: Option<String>,
    error_code: Option<String>,
    error_description: Option<String>,
    connector_txn_id: Option<String>,
    http_code: u16,
    attempt_status: Option<AttemptStatus>,
) -> ErrorResponse {
    ErrorResponse {
        status_code: http_code,
        code: error_code.unwrap_or_else(|| response_code.to_string()),
        message: response_message
            .clone()
            .or_else(|| error_description.clone())
            .unwrap_or_else(|| "Qwikcilver returned a non-zero response code".to_string()),
        reason: error_description.or(response_message),
        attempt_status: attempt_status.map(domain_types::router_data::FlowStatus::Payment),
        connector_transaction_id: connector_txn_id,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}
